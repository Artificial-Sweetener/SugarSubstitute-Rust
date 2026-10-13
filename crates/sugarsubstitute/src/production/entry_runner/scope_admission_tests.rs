// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove actual runner scope routing before any configuration or environment effect.
use super::*;
use std::sync::{
    Condvar, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::{
    configuration::ConfigurationError,
    execution::{AdmissionError, ExecutionError, JobClass, JobOutcome, JobSpec, Operation},
};
use sugarsubstitute_execution::BlockingInputError;
#[derive(Default)]
struct ScopeWake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl ScopeWake {
    fn revision(&self) -> Result<u64, Box<dyn Error>> {
        Ok(*self.revision.lock().map_err(|_| std::io::Error::other("Scope wake poisoned"))?)
    }
    fn wait_after(&self, observed: u64) -> Result<(), Box<dyn Error>> {
        let revision =
            self.revision.lock().map_err(|_| std::io::Error::other("Scope wake poisoned"))?;
        let (current, elapsed) = self
            .changed
            .wait_timeout_while(revision, Duration::from_secs(10), |revision| *revision == observed)
            .map_err(|_| std::io::Error::other("Scope wake poisoned"))?;
        if elapsed.timed_out() && *current == observed {
            return Err("Actual finite completion did not notify its owner".into());
        }
        Ok(())
    }
}
impl CompletionWake for ScopeWake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.revision.lock().map_err(|_| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(std::io::Error::other("Scope wake poisoned")),
            )
        })?;
        *revision = revision.saturating_add(1);
        self.changed.notify_all();
        Ok(())
    }
}

#[derive(Debug)]
struct WakeFailure(Arc<()>);
impl fmt::Display for WakeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Original external completion wake failed")
    }
}
impl Error for WakeFailure {}

struct FailOnceWake {
    observed: ScopeWake,
    pending: AtomicBool,
    identity: Arc<()>,
}
impl CompletionWake for FailOnceWake {
    fn wake(&self) -> Result<(), ExecutionError> {
        self.observed.wake()?;
        if self.pending.swap(false, Ordering::AcqRel) {
            return Err(ExecutionError::new(
                Operation::NativeLifecycle,
                Box::new(WakeFailure(self.identity.clone())),
            ));
        }
        Ok(())
    }
}

#[test]
fn mailbox_wake_failure_retains_original_and_runner_observes_cleanup_after_admission_closes()
-> Result<(), Box<dyn Error>> {
    let invalid = PathBuf::from("relative-installation");
    let command = ProductionCommand {
        installation: invalid.clone(),
        options: super::super::super::options::ProductionEntryOptions {
            configuration: invalid.clone(),
            package: invalid.clone(),
            metadata: invalid.clone(),
            cubes: invalid.clone(),
            script: invalid.clone(),
            profile: invalid.clone(),
        },
    };
    let mut runner = EntryRunner::new(command, invalid, NoChild);
    let identity = Arc::new(());
    let wake = Arc::new(FailOnceWake {
        observed: ScopeWake::default(),
        pending: AtomicBool::new(true),
        identity: identity.clone(),
    });
    let rejected_start = runner.start(wake.clone()).err().ok_or("Invalid setup was admitted")?;
    assert!(rejected_start.downcast_ref::<ConfigurationError>().is_some());
    assert!(runner.failure_source().is_none());
    assert!(!runner.closing);
    let client = runner.client()?.clone();
    let finite = runner.finite.as_ref().ok_or("Actual finite scope missing")?;
    let spec = JobSpec {
        id: runner.jobs.allocate()?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: None,
        reservation_bytes: 4194304,
        replacement: None,
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let mut input = Some(identity.clone());
    let revision = wake.observed.revision()?;
    let mut job = client.submit_blocking_owned_box(finite, spec, &mut input, move |value, _| {
        observed.fetch_add(1, Ordering::AcqRel);
        Ok::<_, std::io::Error>(value)
    })?;
    assert!(input.is_none());
    wake.observed.wait_after(revision)?;
    let completion = job.try_take_completion().ok_or("Actual wake had no typed completion")?;
    assert_eq!(completion.scope_id, finite.id());
    assert_eq!(completion.spec, spec);
    let JobOutcome::Success(value) = &completion.outcome else {
        return Err("Actual finite factory did not succeed".into());
    };
    assert!(Arc::ptr_eq(value.input(), &identity));
    assert_eq!(calls.load(Ordering::Acquire), 1);
    // The worker join establishes that the failed wake has returned and the real
    // mailbox has retained its cause; a signal from inside the wake precedes that.
    drop(runner.execution.take());
    assert_eq!(finite.snapshot().reserved_bytes, 4194304);
    let categorical = runner.drain(NonZeroUsize::MIN).err().ok_or("Mailbox failure was lost")?;
    assert!(categorical.to_string().contains("completion drain failed"));
    assert!(runner.closing);
    assert!(runner.shutdown.is_some());
    let retained = runner.failure_source().ok_or("Original mailbox cause was not retained")?;
    let primary = retained as *const dyn Error;
    assert!(retained.downcast_ref::<ExecutionError>().is_some());
    let mut cause = Some(retained);
    let mut original = false;
    while let Some(current) = cause {
        if let Some(wake_cause) = current.downcast_ref::<WakeFailure>() {
            original = Arc::ptr_eq(&wake_cause.0, &identity);
        }
        cause = current.source();
    }
    assert!(original, "Original owned external wake cause was replaced");
    assert!(!runner.child_closed);
    assert!(!runner.intake_settled);
    assert!(runner.drain(NonZeroUsize::MIN).is_err());
    assert!(runner.failure_reported);
    assert!(runner.child_closed);
    assert!(runner.intake_settled);
    assert_eq!(runner.drain(NonZeroUsize::MIN)?, HeadlessStep::Shutdown);
    assert!(std::ptr::eq(
        primary,
        runner.failure_source().ok_or("Original failure disappeared during cleanup")?
    ));
    let mut rejected_input = Some(identity.clone());
    let rejected_spec = JobSpec { id: runner.jobs.allocate()?, ..spec };
    let rejected = client.submit_blocking_owned_box(
        runner.finite.as_ref().ok_or("Original finite scope missing")?,
        rejected_spec,
        &mut rejected_input,
        |value, _| Ok::<_, std::io::Error>(value),
    );
    assert!(matches!(rejected, Err(BlockingInputError::Admission(AdmissionError::ShuttingDown))));
    assert!(Arc::ptr_eq(rejected_input.as_ref().ok_or("Rejected input was lost")?, &identity));
    drop(completion);
    drop(job);
    assert_eq!(
        runner.finite.as_ref().ok_or("Original finite scope missing")?.snapshot().reserved_bytes,
        0
    );
    assert!(runner.poll_shutdown()?);
    assert!(!runner.finished);
    Ok(())
}
#[test]
fn actual_runner_native_rejects_finite_box_and_real_finite_scope_preserves_original_input()
-> Result<(), Box<dyn Error>> {
    let invalid = PathBuf::from("relative-installation");
    let command = ProductionCommand {
        installation: invalid.clone(),
        options: super::super::super::options::ProductionEntryOptions {
            configuration: invalid.clone(),
            package: invalid.clone(),
            metadata: invalid.clone(),
            cubes: invalid.clone(),
            script: invalid.clone(),
            profile: invalid.clone(),
        },
    };
    let mut runner = EntryRunner::new(command, invalid, NoChild);
    let wake = Arc::new(ScopeWake::default());
    let rejected_start = runner
        .start(wake.clone())
        .err()
        .ok_or("Invalid inert installation entered configuration")?;
    assert!(rejected_start.downcast_ref::<ConfigurationError>().is_some());
    assert!(runner.bootstrap.is_none());
    assert!(runner.authority.is_none());
    assert!(runner.intake.is_none());
    assert!(runner.execution.is_some());
    let client = runner.client.as_ref().ok_or("Actual runner client missing")?;
    let finite = runner.finite.as_ref().ok_or("Actual runner finite scope missing")?;
    let native = runner.native.as_ref().ok_or("Actual runner native scope missing")?;
    let source = runner.source.as_ref().ok_or("Actual source scope missing")?;
    let cleanup = runner.cleanup.as_ref().ok_or("Actual cleanup scope missing")?;
    assert!(source.id() < finite.id() && finite.id() < native.id() && native.id() < cleanup.id());
    assert!(finite.spec().deadline.is_none());
    assert!(native.spec().deadline.is_none());
    assert_eq!(finite.snapshot().reserved_bytes, 0);
    assert_eq!(native.snapshot().reserved_bytes, 0);
    let identity = Arc::new(());
    let mut input = Some(identity.clone());
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let spec = JobSpec {
        id: runner.jobs.allocate()?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: None,
        reservation_bytes: 4194304,
        replacement: None,
    };
    let rejected = client.submit_blocking_owned_box(native, spec, &mut input, move |_, _| {
        observed.fetch_add(1, Ordering::AcqRel);
        Ok::<_, std::io::Error>(())
    });
    assert!(matches!(
        rejected,
        Err(BlockingInputError::Admission(AdmissionError::WrongClass {
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle
        }))
    ));
    assert!(Arc::ptr_eq(input.as_ref().ok_or("Native rejection lost original input")?, &identity));
    assert_eq!(calls.load(Ordering::Acquire), 0);
    assert_eq!(native.snapshot().reserved_bytes, 0);
    let observed = calls.clone();
    let mut job = client.submit_blocking_owned_box(finite, spec, &mut input, move |value, _| {
        observed.fetch_add(1, Ordering::AcqRel);
        Ok::<_, std::io::Error>(value)
    })?;
    assert!(input.is_none());
    let completion = loop {
        let revision = wake.revision()?;
        let batch = client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if batch
            .notifications
            .iter()
            .any(|notice| notice.scope_id == finite.id() && notice.job_id == spec.id)
        {
            break job.try_take_completion().ok_or("Real finite notice had no typed completion")?;
        }
        wake.wait_after(revision)?;
    };
    assert_eq!(completion.scope_id, finite.id());
    assert_eq!(completion.spec, spec);
    let JobOutcome::Success(value) = &completion.outcome else {
        return Err("Actual finite factory failed".into());
    };
    assert!(Arc::ptr_eq(value.input(), &identity));
    assert_eq!(calls.load(Ordering::Acquire), 1);
    drop(completion);
    drop(job);
    drop(runner.execution.take());
    assert_eq!(
        runner.finite.as_ref().ok_or("Original finite scope vanished")?.snapshot().reserved_bytes,
        0
    );
    assert_eq!(
        runner.native.as_ref().ok_or("Original native scope vanished")?.snapshot().reserved_bytes,
        0
    );
    Ok(())
}
