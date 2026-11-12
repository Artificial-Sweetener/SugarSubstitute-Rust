// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the real callback with genuine finite Core contexts and injected clock boundaries.
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
type TestError = Box<dyn Error + Send + Sync>;
#[derive(Debug, Eq, PartialEq)]
struct PanicMarker(u64);
#[derive(Clone, Copy)]
enum Mode {
    Current,
    Cancelled,
    Deadline,
    Panic,
}
#[derive(Default)]
struct ProbeClock {
    worker: Mutex<Option<(u32, Mode)>>,
    global: AtomicU64,
}
impl Clock for ProbeClock {
    fn now(&self) -> Moment {
        let mut worker = self.worker.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if worker.as_ref().is_some_and(|(id, _)| *id == crate::native::current_clock_observer()) {
            let (_, mode) =
                worker.take().unwrap_or((crate::native::current_clock_observer(), Mode::Current));
            match mode {
                Mode::Deadline => {
                    self.global.store(100, Ordering::Release);
                    return Moment::from_nanos(100);
                }
                Mode::Panic => std::panic::resume_unwind(Box::new(PanicMarker(881))),
                _ => {}
            }
        }
        Moment::from_nanos(self.global.load(Ordering::Acquire).max(17))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
struct Probe {
    status: u32,
    code: u32,
    retained: Option<ControlCause>,
    _claim: MemoryClaim,
}
fn observe<T: Send + 'static, E: Send + 'static>(
    client: &ExecutionClient,
    wake: &Wake,
    job: &mut OwnedJob<T, E>,
) -> Result<(JobCompletion<T, E>, Option<ExecutionError>), TestError> {
    let mut fatal = None;
    loop {
        let old = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Err(cause) = client.drain_completions(NonZeroUsize::MIN) {
            assert!(cause.to_string().contains("Non-text panic payload"));
            if fatal.is_none() {
                fatal = Some(cause);
            }
        }
        if let Some(value) = job.try_take_completion() {
            return Ok((value, fatal));
        }
        drop(
            wake.changed
                .wait_while(
                    wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                    |count| *count == old,
                )
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }
}
type Observed = (Probe, JobOutcome<(), TestError>, Option<ExecutionError>, Arc<Scope>);
fn run(mode: Mode) -> Result<Observed, TestError> {
    let clock = Arc::new(ProbeClock::default());
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope =
        Arc::new(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
    let current = scope.clone();
    let recorded = Arc::new(Mutex::new(None));
    let output = recorded.clone();
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: 8 * 1024 * 1024,
            replacement: None,
        },
        move |context| -> Result<(), TestError> {
            let claim = context.memory().claim(16 * 1024)?;
            let mut history = ControlHistory::default();
            let mut invocation = ControlInvocation::new(&context, &mut history);
            if matches!(mode, Mode::Cancelled) {
                current.cancel()
            }
            if matches!(mode, Mode::Deadline | Mode::Panic) {
                *clock.worker.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some((crate::native::current_clock_observer(), mode));
            }
            let (status, code) = {
                let source = invocation.source();
                let mut code = 0;
                // SAFETY: This is the exact live exclusive stack source and writable
                // result used by the synchronous native ABI, on this calling thread.
                let status = unsafe { (source.check)(source.borrowed_state, &mut code) };
                (status, code)
            };
            let retained = invocation.take_failure();
            *output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some(Probe { status, code, retained, _claim: claim });
            Ok(())
        },
    )?;
    let (completion, fatal) = observe(&client, &wake, &mut job)?;
    drop(completion.memory);
    drop(job);
    drop(owner);
    let probe = recorded
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .ok_or("callback did not record")?;
    assert_eq!(scope.snapshot().reserved_bytes, 8 * 1024 * 1024);
    Ok((probe, completion.outcome, fatal, scope))
}
#[test]
fn current_core_context_matches_exact_abi_layout_and_zero_cause() -> Result<(), TestError> {
    let (probe, outcome, fatal, scope) = run(Mode::Current)?;
    assert_eq!((probe.status, probe.code), (CURRENT, 0));
    assert!(probe.retained.is_none());
    assert!(matches!(outcome, JobOutcome::Success(())));
    assert!(fatal.is_none());
    drop(probe);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn actual_scope_cancellation_returns_cancelled_without_claiming_native_stop()
-> Result<(), TestError> {
    let (probe, outcome, fatal, scope) = run(Mode::Cancelled)?;
    assert_eq!((probe.status, probe.code), (CANCELLED, ERROR_CANCELLED));
    assert!(matches!(probe.retained.as_ref(), Some(ControlCause::Cancelled)));
    assert!(!matches!(outcome, JobOutcome::Success(())));
    assert!(fatal.is_none());
    drop(probe);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn deadline_equality_is_expired_under_the_same_original_context() -> Result<(), TestError> {
    let (probe, outcome, fatal, scope) = run(Mode::Deadline)?;
    assert_eq!((probe.status, probe.code), (TIMED_OUT, ERROR_TIMEOUT));
    assert!(matches!(probe.retained.as_ref(), Some(ControlCause::TimedOut)));
    assert!(!matches!(outcome, JobOutcome::Success(())));
    assert!(fatal.is_none());
    drop(probe);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn original_nontext_clock_panic_stays_owned_and_never_unwinds_across_c() -> Result<(), TestError> {
    let (probe, outcome, fatal, scope) = run(Mode::Panic)?;
    assert_eq!((probe.status, probe.code), (FAILED, ERROR_BRIDGE));
    assert_eq!(
        probe
            .retained
            .as_ref()
            .and_then(ControlCause::panic_payload)
            .and_then(|payload| payload.downcast_ref::<PanicMarker>()),
        Some(&PanicMarker(881))
    );
    assert!(matches!(outcome, JobOutcome::Cancelled));
    assert!(fatal.is_some());
    drop(probe);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
