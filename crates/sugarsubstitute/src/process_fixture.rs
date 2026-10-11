// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify three self-executable process families through the real retained owners.
//! This isolated role supplies constant matching facts, not authenticated coordinator
//! identity or installation trust. Only the current EXE and its immediate-exit role
//! are admitted; the native pump creates no application window or presentation.
mod activity;
mod lifetime;
mod progress;
mod receipt;
#[cfg(test)]
mod tests;

use progress::{CaseProgress, StopPlan};
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{
    execution::{
        Clock, ExecutionPort, JobClass, JobId, JobSpec, Moment, Operation, Scope, ScopeId,
        ScopeSpec,
    },
    lifecycle::{
        CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey,
        ShutdownId,
    },
};
use sugarsubstitute_desktop::run_headless;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock, WorkerStatus};
use sugarsubstitute_supervision::{
    FamilyBinding, LaunchEnvironment, WindowsLaunchCommand, WindowsLaunchInput,
    WindowsProcessRegistry,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const CODES: [u32; 3] = [0, 37, 259];

/// Run on the native main thread and return bounded facts after actual final teardown.
/// Errors retain their original typed cause after all admitted families/workers settle.
/// The parent's separate absolute deadline must supervise this qualification process;
/// the inner sixty-second deadline closes admission without inventing settlement.
pub(crate) fn run() -> Result<serde_json::Value> {
    let mut fixture = Fixture::new()?;
    run_headless(&mut fixture)?;
    fixture.receipt()
}

struct Fixture {
    clock: Arc<MonotonicClock>,
    deadline: Instant,
    execution_deadline: Moment,
    deadline_observed: bool,
    owner_key: CoordinatorKey,
    registry: WindowsProcessRegistry,
    owner: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    scope: Option<Scope>,
    next_scope: u64,
    next_job: u64,
    cases: [CaseProgress; 3],
    current: usize,
    stop: Option<StopPlan>,
    cleanup_requested: bool,
    cleanup_job: Option<JobId>,
    closing: bool,
    execution_stopping: bool,
    workers: Option<WorkerStatus>,
    finished: bool,
}
impl Fixture {
    fn new() -> Result<Self> {
        let started = Instant::now();
        let deadline =
            started.checked_add(Duration::from_secs(60)).ok_or("Fixture deadline overflow")?;
        let clock = MonotonicClock::new();
        let remaining =
            deadline.checked_duration_since(Instant::now()).ok_or("Fixture deadline elapsed")?;
        let execution_deadline = clock.now().checked_add(remaining)?;
        // Private fixture facts match within this one registry; they grant no IPC authority.
        let owner_key = CoordinatorKey {
            namespace: NamespaceId::new([0x51; 32]),
            epoch: CoordinatorEpoch::new([0x52; 16])?,
        };
        Ok(Self {
            clock,
            deadline,
            execution_deadline,
            deadline_observed: false,
            owner_key,
            registry: WindowsProcessRegistry::new(owner_key),
            owner: None,
            client: None,
            scope: None,
            next_scope: 2,
            next_job: 1,
            cases: CODES.map(CaseProgress::new),
            current: 0,
            stop: None,
            cleanup_requested: false,
            cleanup_job: None,
            closing: false,
            execution_stopping: false,
            workers: None,
            finished: false,
        })
    }
    fn spec(&mut self, class: JobClass, bytes: u64) -> Result<JobSpec> {
        let id = JobId::new(self.next_job)?;
        self.next_job = self.next_job.checked_add(1).ok_or("Fixture job identity exhausted")?;
        Ok(JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class,
            deadline: None,
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    fn launch_next(&mut self) -> Result<()> {
        if self.closing || self.current == self.cases.len() {
            return Ok(());
        }
        let executable = std::env::current_exe()?;
        let cwd = executable.parent().ok_or("Fixture EXE has no parent directory")?;
        let arguments = [
            "--internal-native-exit-fixture".into(),
            self.cases[self.current].expected.to_string().into(),
        ];
        let command = WindowsLaunchCommand::from_os(&executable, cwd, &arguments)?;
        let input = WindowsLaunchInput::new(
            command,
            LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
        )?;
        let watch = self.spec(JobClass::NativeObservation, 4096)?;
        let creation =
            self.spec(JobClass::InfrastructureLifecycle, input.required_reservation_bytes())?;
        let native_scope = ScopeSpec { id: ScopeId::new(self.next_scope)?, deadline: None };
        self.next_scope =
            self.next_scope.checked_add(1).ok_or("Fixture scope identity exhausted")?;
        let run = RunKey {
            owner: self.owner_key,
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(u64::try_from(self.current)? + 1)?,
        };
        let ticket = self.registry.submit_launch(
            self.client.as_ref().ok_or("Fixture execution client missing")?,
            self.scope.as_ref().ok_or("Fixture creation scope missing")?,
            native_scope,
            creation,
            watch,
            FamilyBinding::Desktop(run),
            input,
        )?;
        self.cases[self.current].bind(ticket.process())?;
        Ok(())
    }
    fn ensure_stop(&mut self) -> Result<()> {
        let Some(key) = self.cases.get(self.current).and_then(|case| case.process) else {
            return Ok(());
        };
        if self.stop.is_none() {
            let plan = StopPlan::new(self.clock.now(), Instant::now())?;
            let shutdown = ShutdownId::new(u64::try_from(self.current)? + 1)?;
            self.registry.begin_stop(key, shutdown, plan.budget)?;
            self.cases[self.current].stop = Some((shutdown, plan.budget));
            self.stop = Some(plan);
            self.cleanup_requested = true;
        }
        Ok(())
    }
    fn drain(&mut self, max: NonZeroUsize) -> Result<()> {
        if let Some(client) = &self.client {
            let batch = client.drain_completions(max)?;
            self.registry.consume(&batch)?;
            let progress =
                batch.notifications.iter().any(|notice| Some(notice.job_id) != self.cleanup_job);
            if batch.notifications.iter().any(|notice| Some(notice.job_id) == self.cleanup_job) {
                self.cleanup_job = None;
                if let Some(key) = self.cases.get(self.current).and_then(|case| case.process)
                    && self.registry.snapshot(key)?.member_watch_required
                {
                    self.cleanup_requested = true;
                }
            }
            if progress && self.stop.is_some() {
                self.cleanup_requested = true;
            }
        }
        Ok(())
    }
}
