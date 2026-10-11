// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe finite production admission and retained native state using controlled external boundaries.
use super::*;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{
        Condvar,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::{ExecutionOwner, MonotonicClock};
pub(super) type TestResult<T = ()> = Result<T, Box<dyn Error>>;
#[derive(Default)]
pub(super) struct Wake {
    pub(super) count: Mutex<usize>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *lock(&self.count) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
impl Wake {
    pub(super) fn wait(&self, before: usize) -> TestResult {
        let (count, timeout) = self
            .changed
            .wait_timeout_while(lock(&self.count), Duration::from_secs(10), |n| *n == before)
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && *count == before {
            return Err("Finite native completion did not arrive".into());
        }
        Ok(())
    }
}
#[derive(Default)]
pub(super) struct Gate {
    open: Mutex<bool>,
    changed: Condvar,
}
impl Gate {
    pub(super) fn open(&self) {
        *lock(&self.open) = true;
        self.changed.notify_all();
    }
    pub(super) fn wait(&self) -> Result<(), RegistryError> {
        let (open, timeout) = self
            .changed
            .wait_timeout_while(lock(&self.open), Duration::from_secs(10), |open| !*open)
            .map_err(|cause| {
                RegistryError::new("controlled_gate", std::io::Error::other(cause.to_string()))
            })?;
        if timeout.timed_out() && !*open {
            return Err(RegistryError::invalid("controlled_gate"));
        }
        Ok(())
    }
}
pub(super) struct Boundary {
    pub(super) creates: AtomicUsize,
    pub(super) closes: AtomicUsize,
    fail_create: bool,
    fail_close: AtomicBool,
    hold: bool,
    entered: Gate,
    release: Gate,
}
impl Boundary {
    pub(super) fn new(fail_create: bool, hold: bool) -> Arc<Self> {
        Arc::new(Self {
            creates: AtomicUsize::new(0),
            closes: AtomicUsize::new(0),
            fail_create,
            fail_close: AtomicBool::new(false),
            hold,
            entered: Gate::default(),
            release: Gate::default(),
        })
    }
}
pub(super) struct Fake(pub(super) Arc<Boundary>);
impl Lease for Fake {
    fn create(
        &mut self,
        input: &mut FixtureDescendantInput,
        _context: &JobContext,
    ) -> Result<FixtureDescendantStarted, RegistryError> {
        self.0.creates.fetch_add(1, Ordering::AcqRel);
        if self.0.fail_create {
            Err(RegistryError::new("create_partial_output", std::io::Error::from_raw_os_error(5)))
        } else {
            Ok(FixtureDescendantStarted {
                correlation: input.correlation,
                process_id: 123,
                creation_time_100ns: 456,
            })
        }
    }
    fn retire(&mut self) -> Result<(), RegistryError> {
        self.0.entered.open();
        if self.0.hold {
            self.0.release.wait()?;
        }
        if self.0.fail_close.load(Ordering::Acquire) {
            return Err(RegistryError::new(
                "close_partial_output",
                std::io::Error::from_raw_os_error(6),
            ));
        }
        self.0.closes.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}
pub(super) struct Rig {
    owner: ExecutionOwner,
    pub(super) client: ExecutionClient,
    pub(super) scope: Scope,
    pub(super) wake: Arc<Wake>,
}
impl Rig {
    pub(super) fn new() -> TestResult<Self> {
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(MonotonicClock::new(), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        Ok(Self { owner, client, scope, wake })
    }
    pub(super) fn completion(
        &self,
        launch: &mut FixtureDescendantLaunch,
    ) -> TestResult<JobCompletion<(), RegistryError>> {
        loop {
            let before = *lock(&self.wake.count);
            let _ = self.client.drain_completions(NonZeroUsize::new(32).ok_or("budget")?)?;
            if let Some(completion) = launch.take_completion() {
                return Ok(completion);
            }
            self.wake.wait(before)?;
        }
    }
}
pub(super) fn specification(id: u64, bytes: u64) -> TestResult<JobSpec> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(u64::MAX)),
        reservation_bytes: bytes,
        replacement: None,
    })
}
pub(super) fn input() -> TestResult<FixtureDescendantInput> {
    Ok(FixtureDescendantInput {
        command: crate::WindowsLaunchCommand::from_utf16(
            &"E:\\fixture.exe".encode_utf16().collect::<Vec<_>>(),
            &"E:\\".encode_utf16().collect::<Vec<_>>(),
            &[],
        )?
        .into_native_parts(),
        module: "E:\\fixture.exe".into(),
        events: None,
        correlation: NativeCorrelation::new(1)?,
        deadline: sugarsubstitute_execution::windows_fixture::FixtureDeadline::new(1000, 1000)?,
        required: 4096,
    })
}
#[test]
fn partial_creation_outputs_remain_charged_and_nonterminal_until_checked_close() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(true, true);
    let mut launch = FixtureDescendantLaunch::submit_with(
        &rig.client,
        &rig.scope,
        specification(1, 4096)?,
        input()?,
        Box::new(Fake(boundary.clone())),
    )?;
    boundary.entered.wait()?;
    assert!(launch.take_completion().is_none());
    assert!(!launch.native_retired());
    assert_eq!(rig.scope.snapshot().running, 1);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 4096);
    boundary.release.open();
    assert!(
        matches!(rig.completion(&mut launch)?.outcome,JobOutcome::Failure(ref cause) if cause.stage()=="create_partial_output")
    );
    assert!(launch.native_retired());
    assert!(launch.take_started().is_none());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn failed_close_retains_original_creation_cause_and_owned_explicit_retirement() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(true, false);
    boundary.fail_close.store(true, Ordering::Release);
    let mut launch = FixtureDescendantLaunch::submit_with(
        &rig.client,
        &rig.scope,
        specification(1, 4096)?,
        input()?,
        Box::new(Fake(boundary.clone())),
    )?;
    let completion = rig.completion(&mut launch)?;
    assert!(
        matches!(completion.outcome,JobOutcome::Failure(ref cause) if cause.stage()=="create_partial_output" && cause.cleanup_causes().len()==1)
    );
    drop(completion);
    assert!(!launch.native_retired());
    assert_eq!(rig.scope.snapshot().reserved_bytes, 4096);
    boundary.fail_close.store(false, Ordering::Release);
    launch.submit_retirement(&rig.client, &rig.scope, specification(2, 4096)?)?;
    assert!(matches!(rig.completion(&mut launch)?.outcome, JobOutcome::Success(())));
    assert!(launch.native_retired());
    Ok(())
}
#[test]
fn admission_rejection_never_runs_creation_boundary() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(false, false);
    let result = FixtureDescendantLaunch::submit_with(
        &rig.client,
        &rig.scope,
        specification(1, 4095)?,
        input()?,
        Box::new(Fake(boundary.clone())),
    );
    assert!(matches!(result, Err(RegistryAdmissionError::InvalidSpecification)));
    assert_eq!(boundary.creates.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    Ok(())
}
#[test]
fn actual_start_is_transferred_once_after_creation_and_closure_terminal() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(false, true);
    let mut launch = FixtureDescendantLaunch::submit_with(
        &rig.client,
        &rig.scope,
        specification(1, 4096)?,
        input()?,
        Box::new(Fake(boundary.clone())),
    )?;
    boundary.entered.wait()?;
    assert!(launch.take_started().is_none());
    boundary.release.open();
    assert!(matches!(rig.completion(&mut launch)?.outcome, JobOutcome::Success(())));
    assert_eq!(launch.take_started().ok_or("actual start")?.process_id, 123);
    assert!(launch.take_started().is_none());
    assert!(launch.native_retired());
    drop(rig.owner);
    Ok(())
}
#[test]
fn exact_role_arguments_preserve_original_deadline_and_four_event_order() -> TestResult {
    let args = input::arguments(
        [11, 12, 13, 14],
        sugarsubstitute_execution::windows_fixture::FixtureDeadline::new(1000, 1000)?,
        NativeCorrelation::new(1)?,
        259,
    );
    assert_eq!(
        args.into_iter()
            .map(|arg| arg.into_string())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "UTF8")?,
        [
            "--internal-native-held-child",
            "--ready",
            "11",
            "--release",
            "12",
            "--challenge",
            "13",
            "--response",
            "14",
            "--deadline-qpc",
            "1000",
            "--qpc-frequency",
            "1000",
            "--correlation",
            "1",
            "--exit-code",
            "259"
        ]
    );
    Ok(())
}
