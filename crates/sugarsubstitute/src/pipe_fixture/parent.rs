// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose one fixed campaign with retained execution, authenticated channels and registry.
use super::{Result, evidence::Case, options::CaseKind};
use std::{
    ffi::OsString,
    os::windows::io::AsRawHandle,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{
    execution::{Clock, JobClass, JobId, JobSpec, Moment, Operation, Scope, ScopeId, ScopeSpec},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
};
use sugarsubstitute_desktop::run_headless;
use sugarsubstitute_execution::windows_lifecycle_pipe::MIN_LIFECYCLE_PIPE_BYTES;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock, WorkerStatus};
use sugarsubstitute_supervision::{
    FamilyBinding, LaunchEnvironment, WindowsLaunchCommand, WindowsLaunchInput,
    WindowsProcessRegistry,
};

pub(super) fn run() -> Result<serde_json::Value> {
    let mut parent = Parent::new()?;
    run_headless(&mut parent)?;
    parent.receipt()
}
pub(super) struct Parent {
    pub(super) clock: Arc<MonotonicClock>,
    pub(super) deadline: Instant,
    pub(super) execution_deadline: Moment,
    pub(super) deadline_observed: bool,
    pub(super) registry: Option<WindowsProcessRegistry>,
    pub(super) registry_creation: Option<crate::process_fixture::registry::PendingRegistry>,
    pub(super) owner: Option<ExecutionOwner>,
    pub(super) client: Option<ExecutionClient>,
    pub(super) scope: Option<Scope>,
    pub(super) pipe_scope: Option<Scope>,
    pub(super) next_scope: u64,
    pub(super) next_job: u64,
    pub(super) cases: [Case; 3],
    pub(super) current: usize,
    pub(super) closing: bool,
    pub(super) execution_stopping: bool,
    pub(super) workers: Option<WorkerStatus>,
    pub(super) finished: bool,
}
impl Parent {
    pub(super) fn new() -> Result<Self> {
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(60))
            .ok_or("Lifecycle campaign deadline overflow")?;
        let clock = MonotonicClock::new();
        let execution_deadline = original_deadline(clock.as_ref(), deadline, Instant::now)?;
        // Isolated local facts bind only this campaign's actual contained children.
        let owner_key = CoordinatorKey {
            namespace: NamespaceId::new([0x71; 32]),
            epoch: CoordinatorEpoch::new([0x72; 16])?,
        };
        let run = |id| -> Result<RunKey> {
            Ok(RunKey {
                owner: owner_key,
                instance: InstanceId::new(id)?,
                generation: RunGeneration::new(1)?,
            })
        };
        Ok(Self {
            clock,
            deadline,
            execution_deadline,
            deadline_observed: false,
            registry: None,
            registry_creation: None,
            owner: None,
            client: None,
            scope: None,
            pipe_scope: None,
            next_scope: 3,
            next_job: 1,
            cases: [
                Case::new(CaseKind::Valid, run(1)?),
                Case::new(CaseKind::WrongCapability, run(2)?),
                Case::new(CaseKind::ForeignGeneration, run(3)?),
            ],
            current: 0,
            closing: false,
            execution_stopping: false,
            workers: None,
            finished: false,
        })
    }
    pub(super) fn spec(&mut self, class: JobClass, bytes: u64) -> Result<JobSpec> {
        let id = JobId::new(self.next_job)?;
        self.next_job = self.next_job.checked_add(1).ok_or("Lifecycle job identity exhausted")?;
        Ok(JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class,
            deadline: None,
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    pub(super) fn reserve_next(&mut self) -> Result<()> {
        if self.closing || self.current == self.cases.len() {
            return Ok(());
        }
        let spec = self.spec(JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
        let session =
            self.client.as_ref().ok_or("Lifecycle client missing")?.reserve_lifecycle_pipe_server(
                self.pipe_scope.as_ref().ok_or("Lifecycle pipe scope missing")?,
                spec,
                self.cases[self.current].run,
                self.execution_deadline,
            )?;
        self.cases[self.current].session = Some(session);
        Ok(())
    }
    /// Pass only a nonsecret section token/deadline/mode to the dedicated self child.
    pub(super) fn launch(&mut self) -> Result<()> {
        let case = &mut self.cases[self.current];
        let control = &case.session.as_ref().ok_or("Lifecycle prepared session missing")?.control;
        let enrollment = control.take_desktop_enrollment()?;
        let bootstrap = control.take_bootstrap()?.ok_or("Lifecycle prepared section missing")?;
        let deadline = bootstrap.deadline().ok_or("Lifecycle bootstrap deadline missing")?;
        let section = bootstrap.borrow_inheritance().section.as_raw_handle() as usize as u64;
        let arguments: Vec<OsString> = [
            "--internal-native-pipe-child".to_owned(),
            "--section".to_owned(),
            section.to_string(),
            "--deadline-qpc".to_owned(),
            deadline.ticks().to_string(),
            "--qpc-frequency".to_owned(),
            deadline.frequency().to_string(),
            "--case".to_owned(),
            case.kind.name().to_owned(),
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        let executable = std::env::current_exe()?;
        let cwd = executable.parent().ok_or("Lifecycle self EXE has no parent")?;
        let command = WindowsLaunchCommand::from_os(&executable, cwd, &arguments)?;
        let input = WindowsLaunchInput::with_lifecycle_pipe(
            command,
            LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
            bootstrap,
            enrollment,
        )?;
        case.bootstrap_deadline = Some(deadline);
        let run = case.run;
        let watch = self.spec(JobClass::NativeObservation, 4096)?;
        let creation =
            self.spec(JobClass::InfrastructureLifecycle, input.required_reservation_bytes())?;
        let native_spec = ScopeSpec { id: ScopeId::new(self.next_scope)?, deadline: None };
        self.next_scope =
            self.next_scope.checked_add(1).ok_or("Lifecycle native scope identity exhausted")?;
        let ticket =
            self.registry.as_mut().ok_or("Actual registry is not prepared")?.submit_launch(
                self.client.as_ref().ok_or("Lifecycle client missing")?,
                self.scope.as_ref().ok_or("Lifecycle creation scope missing")?,
                native_spec,
                creation,
                watch,
                FamilyBinding::Desktop(run),
                input,
            )?;
        self.cases[self.current].process = Some(ticket.process());
        self.cases[self.current].creation_job = Some(creation.id);
        Ok(())
    }
}

/// Sample the destination epoch first so sampling latency cannot extend the source deadline.
fn original_deadline(
    clock: &dyn Clock,
    deadline: Instant,
    native_now: impl FnOnce() -> Instant,
) -> Result<Moment> {
    let before = clock.now();
    let remaining = deadline
        .checked_duration_since(native_now())
        .ok_or("Lifecycle campaign already expired")?;
    Ok(before.checked_add(remaining)?)
}

#[cfg(test)]
#[path = "clock_tests.rs"]
mod clock_tests;
