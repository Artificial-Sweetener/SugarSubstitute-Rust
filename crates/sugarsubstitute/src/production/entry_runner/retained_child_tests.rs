// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retained fatal child sources enter actual runner cleanup instead of vetoing idle forever.
use super::*;
use sugarsubstitute_contracts::{configuration::ConfigurationError, execution::ExecutionError};

#[derive(Debug)]
struct OriginalChildFailure(Arc<()>);
impl fmt::Display for OriginalChildFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Original retained child failure")
    }
}
impl Error for OriginalChildFailure {}
struct RetainedChild {
    identity: Arc<()>,
    failure: Option<Box<dyn Error>>,
    budget: Option<ShutdownBudget>,
    close_calls: usize,
    cleanup_ready: bool,
    retain_on_poll: bool,
    fail_cleanup: bool,
    finish_calls: usize,
}
impl RetainedChild {
    /// Retain the same external child cause through activity and cleanup observations.
    fn retain_failure(&mut self) {
        self.failure.get_or_insert_with(|| Box::new(OriginalChildFailure(self.identity.clone())));
    }
    /// Model an external child boundary retaining its error while returning Active.
    fn active_turn(&mut self) -> HeadlessStep {
        self.retain_failure();
        HeadlessStep::Active
    }
}
impl EntryActivation for RetainedChild {
    fn enter_activation(
        &mut self,
        _: &mut Option<AuthenticatedEntryLaunch>,
        _: EntryResources<'_>,
    ) -> Result<(), Box<dyn Error>> {
        Err("No test child admission exists".into())
    }
    fn consume(
        &mut self,
        _: &CompletionBatch,
        _: EntryResources<'_>,
    ) -> Result<HeadlessStep, Box<dyn Error>> {
        Ok(self.active_turn())
    }
    fn request_close(&mut self, budget: Option<ShutdownBudget>) -> Result<(), Box<dyn Error>> {
        assert_eq!(budget, self.budget);
        self.close_calls += 1;
        Ok(())
    }
    fn shutdown_budget(&self) -> Option<ShutdownBudget> {
        self.budget
    }
    fn normal_close_requested(&self) -> bool {
        false
    }
    fn failure_source(&self) -> Option<&(dyn Error + 'static)> {
        self.failure.as_deref()
    }
    fn next_deadline(&self) -> Option<Instant> {
        None
    }
    fn poll_close(
        &mut self,
        _: &CompletionBatch,
        _: Option<EntryResources<'_>>,
        budget: ShutdownBudget,
    ) -> Result<bool, Box<dyn Error>> {
        assert_eq!(Some(budget), self.budget);
        if self.retain_on_poll {
            self.retain_failure();
        }
        if self.fail_cleanup {
            return Err("Persistent failed external cleanup".into());
        }
        Ok(self.cleanup_ready)
    }
    fn finish(&mut self) -> Result<(), Box<dyn Error>> {
        assert!(self.cleanup_ready);
        self.finish_calls += 1;
        Ok(())
    }
}
struct QuietWake;

#[test]
fn expired_failed_cleanup_returns_after_actual_execution_join_instead_of_hiding_forever()
-> Result<(), Box<dyn Error>> {
    let mut runner = runner()?;
    runner.child.retain_failure();
    runner.child.fail_cleanup = true;
    runner.request_shutdown()?;
    runner.cleanup_deadline = Some(Instant::now());
    assert!(runner.poll_shutdown().is_err());
    let until = Instant::now().checked_add(Duration::from_secs(15)).ok_or("Guard overflow")?;
    while !runner.poll_shutdown()? {
        if Instant::now() >= until {
            return Err("Actual execution workers did not settle".into());
        }
        std::hint::spin_loop();
    }
    assert!(!runner.child.cleanup_ready);
    assert!(!runner.child_closed);
    assert!(runner.execution.as_ref().ok_or("Execution absent")?.poll_shutdown()?.settled);
    assert!(runner.finish_shutdown().is_err());
    assert_eq!(runner.child.finish_calls, 0);
    assert!(runner.failure_source().is_some());
    Ok(())
}

impl CompletionWake for QuietWake {
    fn wake(&self) -> Result<(), ExecutionError> {
        Ok(())
    }
}
/// Retain the existing actual execution/scopes before invalid configuration effects.
fn runner() -> Result<EntryRunner<RetainedChild>, Box<dyn Error>> {
    let invalid = PathBuf::from("relative-installation");
    let command = ProductionCommand {
        launch_selection: None,
        diagnostics: Default::default(),
        installation: invalid.clone(),
        options: super::unopened_paths()?.into(),
    };
    let identity = Arc::new(());
    let child = RetainedChild {
        identity: identity.clone(),
        failure: None,
        budget: None,
        close_calls: 0,
        cleanup_ready: false,
        retain_on_poll: false,
        fail_cleanup: false,
        finish_calls: 0,
    };
    let mut runner = EntryRunner::new(
        command,
        invalid,
        Box::new(child),
        crate::diagnostics::DiagnosticHealth::fixture(),
    );
    // The existing invalid-start fixture retains the real execution/scopes before configuration effects.
    let rejected = runner.start(Arc::new(QuietWake)).err().ok_or("Invalid setup was admitted")?;
    assert!(rejected.downcast_ref::<ConfigurationError>().is_some());
    assert!(runner.bootstrap.is_none());
    let budget = ShutdownBudget::new(runner.client()?.now()?)?;
    runner.child.budget = Some(budget);
    Ok(runner)
}
/// Observe actual child closure and adapter joins without assuming elapsed native time.
fn finish_cleanup(
    runner: &mut EntryRunner<RetainedChild>,
    original: *const dyn Error,
) -> Result<(), Box<dyn Error>> {
    runner.child.cleanup_ready = true;
    assert_eq!(runner.drain(NonZeroUsize::MIN)?, HeadlessStep::Shutdown);
    assert!(runner.child_closed);
    assert!(runner.intake_settled);
    assert!(!runner.finished);
    let until = runner.cleanup_deadline.ok_or("Original cleanup deadline missing")?;
    while !runner.poll_shutdown()? {
        if Instant::now() >= until {
            return Err("Original Runner cleanup failed to observe actual worker settlement".into());
        }
        std::hint::spin_loop();
    }
    let actual =
        runner.execution.as_ref().ok_or("Execution was dropped before joins")?.poll_shutdown()?;
    assert!(actual.settled);
    assert!(actual.unresolved_job_ids.is_empty());
    assert!(
        actual.workers.async_finished
            && actual.workers.cpu_finished
            && actual.workers.native_finished
    );
    runner.finish_shutdown()?;
    assert!(runner.execution.is_none());
    assert!(runner.finished);
    assert_eq!(runner.child.finish_calls, 1);
    assert_eq!(runner.child.close_calls, 1);
    assert!(std::ptr::eq(
        original,
        runner.failure_source().ok_or("Original source disappeared during cleanup")?
    ));
    Ok(())
}
#[test]
fn active_child_retained_failure_starts_bounded_runner_cleanup_and_preserves_original_source()
-> Result<(), Box<dyn Error>> {
    let mut runner = runner()?;
    let budget = runner.child.budget;
    assert_eq!(runner.child.active_turn(), HeadlessStep::Active);
    let source = runner.child.failure_source().ok_or("Child did not retain its original source")?;
    let original = source as *const dyn Error;
    assert!(Arc::ptr_eq(
        &source.downcast_ref::<OriginalChildFailure>().ok_or("Child cause changed")?.0,
        &runner.child.identity
    ));
    let categorical =
        runner.drain(NonZeroUsize::MIN).err().ok_or("Retained fatal child remained active")?;
    assert!(categorical.to_string().contains("completion routing failed"));
    assert!(matches!(runner.first_origin, Some(FailureOrigin::Child)));
    assert!(runner.closing);
    assert_eq!(runner.shutdown, budget);
    assert!(runner.cleanup_deadline.is_some());
    assert_eq!(runner.child.close_calls, 1);
    assert!(!runner.child_closed);
    assert!(!runner.finished);
    assert!(std::ptr::eq(
        original,
        runner.failure_source().ok_or("Original child source was lost")?
    ));
    assert!(runner.drain(NonZeroUsize::MIN).is_err());
    assert!(runner.failure_reported);
    assert_eq!(runner.drain(NonZeroUsize::MIN)?, HeadlessStep::Shutdown);
    assert!(!runner.child_closed);
    assert!(!runner.finished);
    assert_eq!(runner.shutdown, budget);
    assert_eq!(runner.child.close_calls, 1);
    finish_cleanup(&mut runner, original)?;
    assert_eq!(runner.shutdown, budget);
    assert!(
        format!(
            "{:?}",
            FailureDisplay(runner.failure_source().ok_or("Original diagnostic source missing")?)
        )
        .contains("Original retained child failure")
    );
    Ok(())
}
#[test]
fn closing_child_retained_failure_is_reported_once_before_actual_cleanup()
-> Result<(), Box<dyn Error>> {
    let mut runner = runner()?;
    runner.request_shutdown()?;
    assert!(runner.failure_source().is_none());
    runner.child.retain_on_poll = true;
    assert!(
        runner.drain(NonZeroUsize::MIN).is_err(),
        "Closing child failure was accepted as ordinary settlement"
    );
    assert!(matches!(runner.first_origin, Some(FailureOrigin::Child)));
    assert!(runner.failure_reported);
    assert!(!runner.child_closed);
    assert!(!runner.finished);
    let original =
        runner.child.failure_source().ok_or("Original cleanup source missing")? as *const dyn Error;
    assert!(std::ptr::eq(
        original,
        runner.failure_source().ok_or("Cleanup failure origin changed")?
    ));
    let secondary = runner.secondary.iter().flatten().count();
    assert_eq!(runner.drain(NonZeroUsize::MIN)?, HeadlessStep::Shutdown);
    assert_eq!(runner.secondary.iter().flatten().count(), secondary);
    assert!(!runner.child_closed);
    finish_cleanup(&mut runner, original)
}
#[test]
fn later_child_cleanup_failure_preserves_the_first_owned_origin() -> Result<(), Box<dyn Error>> {
    let mut runner = runner()?;
    let identity = Arc::new(());
    runner.close_on_failure(Box::new(OriginalChildFailure(identity.clone())));
    let original = runner.failure_source().ok_or("First owner source missing")? as *const dyn Error;
    runner.child.retain_on_poll = true;
    assert!(runner.drain(NonZeroUsize::MIN).is_err());
    assert!(matches!(runner.first_origin, Some(FailureOrigin::Owned)));
    let retained = runner.failure_source().ok_or("First owner source replaced")?;
    assert!(std::ptr::eq(original, retained));
    assert!(Arc::ptr_eq(
        &retained.downcast_ref::<OriginalChildFailure>().ok_or("First cause type changed")?.0,
        &identity
    ));
    assert!(runner.child.failure_source().is_some());
    assert!(!runner.child_closed);
    assert_eq!(runner.drain(NonZeroUsize::MIN)?, HeadlessStep::Shutdown);
    finish_cleanup(&mut runner, original)
}

#[derive(Debug)]
enum FundedFiniteFailure {
    Memory(sugarsubstitute_contracts::execution::MemoryClaimError),
    Source { identity: Arc<()>, _storage: sugarsubstitute_contracts::execution::MemoryClaim },
}
impl fmt::Display for FundedFiniteFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Original finite source failure")
    }
}
impl Error for FundedFiniteFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(cause) => Some(cause),
            Self::Source { .. } => None,
        }
    }
}

#[test]
fn failed_finite_source_remains_funded_through_runner_poll_and_actual_worker_joins()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::execution::{JobClass, JobOutcome, JobSpec, Operation};
    let mut runner = runner()?;
    let client = runner.client()?.clone();
    let identity = Arc::new(());
    let retained_identity = identity.clone();
    let spec = JobSpec {
        id: runner.jobs.allocate()?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(runner.child.budget.ok_or("Original budget missing")?.settle_until),
        reservation_bytes: 4096,
        replacement: None,
    };
    assert!(size_of::<FundedFiniteFailure>() <= spec.reservation_bytes as usize);
    let mut job = client.submit_blocking(
        runner.finite.as_ref().ok_or("Original finite scope missing")?,
        spec,
        move |context| {
            let storage = context.memory().claim(4096).map_err(FundedFiniteFailure::Memory)?;
            Err::<(), _>(FundedFiniteFailure::Source {
                identity: retained_identity,
                _storage: storage,
            })
        },
    )?;
    let remaining = spec
        .deadline
        .ok_or("Original finite deadline missing")?
        .checked_duration_since(client.now()?)?;
    let until =
        Instant::now().checked_add(remaining).ok_or("Original finite result wait overflow")?;
    let actual = loop {
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(actual) = job.try_take_completion() {
            break actual;
        }
        if Instant::now() >= until {
            return Err("Original finite source did not deliver its actual failure".into());
        }
        std::hint::spin_loop();
    };
    assert_eq!(actual.scope_id, runner.finite.as_ref().ok_or("Finite scope lost")?.id());
    assert_eq!(actual.spec, spec);
    let JobOutcome::Failure(cause) = actual.outcome else {
        return Err("Actual finite source failure was replaced".into());
    };
    let FundedFiniteFailure::Source { identity: observed, .. } = &cause else {
        return Err("Original reservation could not fund its source".into());
    };
    assert!(Arc::ptr_eq(observed, &identity));
    runner.child.failure = Some(Box::new(cause));
    let original =
        runner.child.failure_source().ok_or("Actual source missing")? as *const dyn Error;
    assert!(runner.drain(NonZeroUsize::MIN).is_err());
    assert!(runner.closing);
    assert!(runner.execution.is_some());
    assert!(!runner.child_closed);
    assert!(!runner.finished);
    let budget = runner.shutdown;
    assert!(runner.drain(NonZeroUsize::MIN).is_err());
    assert_eq!(runner.drain(NonZeroUsize::MIN)?, HeadlessStep::Shutdown);
    finish_cleanup(&mut runner, original)?;
    assert_eq!(runner.shutdown, budget);
    drop(actual.memory);
    drop(job);
    let scope = runner.finite.take().ok_or("Original finite scope lost after joins")?;
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    assert!(std::ptr::eq(
        original,
        runner.failure_source().ok_or("Original source lost after joins")?
    ));
    drop(runner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
