// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit independent capture, authentication, transcript storage and contained launches.
use super::{
    Result,
    channel::Channel,
    evidence::{Case, STORE_BYTES, Store},
    options::Role,
};
use std::{
    ffi::OsString,
    io,
    os::windows::io::AsRawHandle,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{
    execution::{
        Clock, CompletionWake, ExecutionPort, JobClass, JobId, JobSpec, Moment, Operation, Scope,
        ScopeId, ScopeSpec,
    },
    lifecycle::*,
};
use sugarsubstitute_desktop::run_headless;
use sugarsubstitute_execution::{
    ExecutionClient, ExecutionOwner, MonotonicClock, WorkerStatus,
    windows_lifecycle_pipe::MIN_LIFECYCLE_PIPE_BYTES, windows_output::MIN_WINDOWS_OUTPUT_BYTES,
};
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
    pub(super) original: Moment,
    pub(super) deadline_observed: bool,
    pub(super) registry: WindowsProcessRegistry,
    pub(super) owner: Option<ExecutionOwner>,
    pub(super) client: Option<ExecutionClient>,
    pub(super) wake: Option<Arc<dyn CompletionWake>>,
    pub(super) scope: Option<Scope>,
    pub(super) native_scope: Option<Scope>,
    pub(super) next_scope: u64,
    pub(super) next_job: u64,
    pub(super) cases: [Case; 4],
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
            .ok_or("Output campaign deadline overflow")?;
        let clock = MonotonicClock::new();
        let before = clock.now();
        let original = before.checked_add(
            deadline
                .checked_duration_since(Instant::now())
                .ok_or("Output campaign expired before admission")?,
        )?;
        let owner = CoordinatorKey {
            namespace: NamespaceId::new([0x81; 32]),
            epoch: CoordinatorEpoch::new([0x82; 16])?,
        };
        let case = |role, id| -> Result<Case> {
            Ok(Case::new(
                role,
                ResourceKey {
                    owner,
                    id: ResourceId::new(id)?,
                    generation: ResourceGeneration::new(1)?,
                },
                RunKey {
                    owner,
                    instance: InstanceId::new(id)?,
                    generation: RunGeneration::new(1)?,
                },
            ))
        };
        Ok(Self {
            clock,
            deadline,
            original,
            deadline_observed: false,
            registry: WindowsProcessRegistry::new(owner)?,
            owner: None,
            client: None,
            wake: None,
            scope: None,
            native_scope: None,
            next_scope: 3,
            next_job: 1,
            cases: [
                case(Role::Corpus, 1)?,
                case(Role::Flood, 2)?,
                case(Role::Idle, 3)?,
                case(Role::Forward, 4)?,
            ],
            current: 0,
            closing: false,
            execution_stopping: false,
            workers: None,
            finished: false,
        })
    }
    pub(super) fn spec(&mut self, class: JobClass, bytes: u64, deadline: bool) -> Result<JobSpec> {
        let id = JobId::new(self.next_job)?;
        self.next_job =
            self.next_job.checked_add(1).ok_or("Output fixture job identity exhausted")?;
        Ok(JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class,
            deadline: deadline.then_some(self.original),
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    pub(super) fn reserve_next(&mut self) -> Result<()> {
        if self.closing || self.current == self.cases.len() {
            return Ok(());
        }
        let spec = self.spec(JobClass::NativeObservation, MIN_WINDOWS_OUTPUT_BYTES, true)?;
        let resource = self.cases[self.current].resource;
        let client = self.client.as_ref().ok_or("Output execution missing")?;
        let session = client.reserve_windows_output(
            self.native_scope.as_ref().ok_or("Output native scope missing")?,
            spec,
            resource,
        )?;
        self.cases[self.current].output = Some(session);
        if self.cases[self.current].role.authenticated() {
            let spec = self.spec(JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES, false)?;
            let case = &mut self.cases[self.current];
            let session = self
                .client
                .as_ref()
                .ok_or("Output execution missing")?
                .reserve_lifecycle_pipe_server(
                    self.native_scope.as_ref().ok_or("Output native scope missing")?,
                    spec,
                    case.run,
                    self.original,
                )?;
            case.channel = Some(Channel::new(session, case.role, Some(case.run)));
        }
        let spec = self.spec(JobClass::InfrastructureLifecycle, STORE_BYTES, true)?;
        self.cases[self.current].store_job =
            Some(self.client.as_ref().ok_or("Output execution missing")?.submit_blocking(
                self.scope.as_ref().ok_or("Output application scope missing")?,
                spec,
                move |context| {
                    let claim = context.memory().claim(STORE_BYTES).map_err(io::Error::other)?;
                    Ok::<_, io::Error>(Store::new(resource, claim))
                },
            )?);
        Ok(())
    }
    pub(super) fn launch(&mut self) -> Result<()> {
        let case = &mut self.cases[self.current];
        let mut args: Vec<OsString> =
            ["--internal-native-output-child", "--case", case.role.name()]
                .into_iter()
                .map(OsString::from)
                .collect();
        let lifecycle = if let Some(channel) = &case.channel {
            let control = channel.control();
            let enrollment = control.take_desktop_enrollment()?;
            let bootstrap =
                control.take_bootstrap()?.ok_or("Output lifecycle bootstrap not prepared")?;
            let deadline =
                bootstrap.deadline().ok_or("Output original bootstrap deadline missing")?;
            let token = bootstrap.borrow_inheritance().section.as_raw_handle() as usize as u64;
            args.extend([
                "--section".into(),
                token.to_string().into(),
                "--deadline-qpc".into(),
                deadline.ticks().to_string().into(),
                "--qpc-frequency".into(),
                deadline.frequency().to_string().into(),
            ]);
            Some((bootstrap, enrollment))
        } else {
            None
        };
        let executable = std::env::current_exe()?;
        let command = WindowsLaunchCommand::from_os(
            &executable,
            executable.parent().ok_or("Output self EXE cwd missing")?,
            &args,
        )?;
        let environment = LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?;
        let input = if let Some((bootstrap, enrollment)) = lifecycle {
            WindowsLaunchInput::with_lifecycle_pipe(command, environment, bootstrap, enrollment)?
        } else {
            WindowsLaunchInput::new(command, environment)?
        };
        let binding = if case.role.authenticated() {
            FamilyBinding::Desktop(case.run)
        } else {
            FamilyBinding::Managed(case.resource)
        };
        let control = &case.output.as_ref().ok_or("Output prepared session missing")?.control;
        let input = input.with_owned_output(
            binding,
            case.resource,
            control,
            control.take_child_stdio()?,
        )?;
        let watch = self.spec(JobClass::NativeObservation, 4096, false)?;
        let creation =
            self.spec(JobClass::InfrastructureLifecycle, input.required_reservation_bytes(), true)?;
        let native_spec = ScopeSpec { id: ScopeId::new(self.next_scope)?, deadline: None };
        self.next_scope = self.next_scope.checked_add(1).ok_or("Output native scope exhausted")?;
        let ticket = self.registry.submit_launch(
            self.client.as_ref().ok_or("Output launch client missing")?,
            self.scope.as_ref().ok_or("Output launch scope missing")?,
            native_spec,
            creation,
            watch,
            binding,
            input,
        )?;
        let case = &mut self.cases[self.current];
        case.process = Some(ticket.process());
        if let Some(channel) = &mut case.channel {
            channel.expected_process = Some(ticket.process());
        }
        Ok(())
    }
}
