// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check reentrant provider observation and unwind-safe ownership during actual retirement.
use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize};

struct RetirementClock {
    target: Mutex<Option<Shared>>,
    observations: AtomicUsize,
    contended: AtomicUsize,
    panic_detached: AtomicBool,
    native_calls: AtomicUsize,
}
impl Clock for RetirementClock {
    #[expect(
        clippy::panic,
        reason = "Exercise retained native custody across an intentional clock provider unwind"
    )]
    fn now(&self) -> Moment {
        let target = self.target.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        if let Some(shared) = target {
            self.observations.fetch_add(1, Ordering::SeqCst);
            if !super::super::fixture::native_probe::is_current(self as *const Self as usize) {
                return Moment::from_nanos(17);
            }
            let detached = match shared.inner.try_lock() {
                Ok(state) => state.port.is_none(),
                Err(std::sync::TryLockError::Poisoned(cause)) => cause.into_inner().port.is_none(),
                Err(std::sync::TryLockError::WouldBlock) => {
                    self.contended.fetch_add(1, Ordering::SeqCst);
                    false
                }
            };
            if detached && self.panic_detached.swap(false, Ordering::SeqCst) {
                panic!("Retirement provider boundary failed while native custody was detached");
            }
        }
        Moment::from_nanos(17)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
impl super::super::fixture::native_probe::NativeControlProbe for RetirementClock {
    fn before_context(&self) {
        self.native_calls.fetch_add(1, Ordering::SeqCst);
    }
}
struct ClockTarget(Arc<RetirementClock>);
impl Drop for ClockTarget {
    fn drop(&mut self) {
        let previous =
            self.0.target.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
        drop(previous);
    }
}
fn retirement_job(
    work: impl FnOnce(JobContext, Arc<RetirementClock>) -> TestResult + Send + 'static,
) -> TestResult {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(RetirementClock {
        target: Mutex::new(None),
        observations: AtomicUsize::new(0),
        contended: AtomicUsize::new(0),
        panic_detached: AtomicBool::new(false),
        native_calls: AtomicUsize::new(0),
    });
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: DIAGNOSTIC_STORAGE_BYTES,
            replacement: None,
        },
        move |context| work(context, clock),
    )?;
    let outcome = observe_job(&client, &wake, &mut job)?;
    match outcome {
        JobOutcome::Success(()) => (),
        other => return Err(format!("Retirement CPU job failed: {other:?}").into()),
    }
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn retirement_provider_reenters_state_without_holding_storage_mutex() -> TestResult {
    retirement_job(|context, clock| {
        let filesystem = Filesystem::new();
        filesystem.probe_native_context(clock.clone());
        let (mut storage, operation) = setup(&context, filesystem)?;
        *clock.target.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(storage.shared.clone());
        let target = ClockTarget(clock.clone());
        storage.retire(&operation, &context)?;
        assert!(clock.observations.load(Ordering::SeqCst) >= 3);
        assert_eq!(clock.contended.load(Ordering::SeqCst), 0);
        assert!(!storage.snapshot().retirement_in_progress());
        assert_eq!(storage.snapshot().live_handles(), 0);
        drop(target);
        drop(operation);
        drop(storage);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}
#[test]
fn normal_native_provider_reenters_state_without_holding_storage_mutex() -> TestResult {
    retirement_job(|context, clock| {
        let filesystem = Filesystem::new();
        filesystem.probe_native_context(clock.clone());
        filesystem.put(DiagnosticObject::Seed(incident()?), b"actual-source");
        filesystem.observe_context_after_open();
        let (mut storage, operation) = setup(&context, filesystem)?;
        *clock.target.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(storage.shared.clone());
        let target = ClockTarget(clock.clone());
        let mut read =
            storage.read(DiagnosticObject::Seed(incident()?), 4096, &operation, &context)?;
        read.retire(&context)?;
        assert!(clock.observations.load(Ordering::SeqCst) >= 3);
        assert_eq!(clock.contended.load(Ordering::SeqCst), 0);
        storage.retire(&operation, &context)?;
        drop(read);
        drop(target);
        drop(operation);
        drop(storage);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}
struct FatalRetained {
    storage: DiagnosticStorage,
    operation: DiagnosticStorageOperation,
    filesystem: Filesystem,
    context: JobContext,
    unwound: bool,
    before: usize,
}
fn provider_unwind_run(cleanup: bool) -> TestResult {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(RetirementClock {
        target: Mutex::new(None),
        observations: AtomicUsize::new(0),
        contended: AtomicUsize::new(0),
        panic_detached: AtomicBool::new(false),
        native_calls: AtomicUsize::new(0),
    });
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let retained = Arc::new(Mutex::new(None));
    let output = retained.clone();
    let producer_clock = clock.clone();
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: DIAGNOSTIC_STORAGE_BYTES,
            replacement: None,
        },
        move |context| -> TestResult {
            let filesystem = Filesystem::new();
            filesystem.probe_native_context(producer_clock.clone());
            let object = DiagnosticObject::Seed(incident()?);
            filesystem.put(object, b"actual-source");
            filesystem.observe_context_after_open();
            let (mut storage, operation) = setup(&context, filesystem.clone())?;
            if cleanup {
                drop(storage.read(object, 4096, &operation, &context)?);
            }
            let before = storage.snapshot().live_handles();
            *producer_clock.target.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some(storage.shared.clone());
            let target = ClockTarget(producer_clock.clone());
            producer_clock.panic_detached.store(true, Ordering::SeqCst);
            let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if cleanup {
                    storage.retire(&operation, &context)
                } else {
                    storage.read(object, 4096, &operation, &context).map(drop)
                }
            }));
            drop(target);
            *output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some(FatalRetained {
                    storage,
                    operation,
                    filesystem,
                    context,
                    unwound: failure.is_err(),
                    before,
                });
            Ok(())
        },
    )?;
    // A fatal provider cannot be sampled again by the completion observer.
    let completion = loop {
        let old = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        drop(
            wake.changed
                .wait_while(
                    wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                    |count| *count == old,
                )
                .map_err(|cause| std::io::Error::other(cause.to_string()))?,
        );
    };
    assert!(matches!(completion.outcome, JobOutcome::Cancelled), "{:?}", completion.outcome);
    let fatal = client.now().err().ok_or("Fatal provider returned renewed control")?;
    assert_eq!(fatal.operation(), Operation::Infrastructure);
    assert!(
        std::error::Error::source(&fatal)
            .is_some_and(|cause| cause.to_string().contains("Retirement provider boundary failed"))
    );
    let FatalRetained { storage, operation, mut filesystem, context, unwound, before } = retained
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .ok_or("Actual fatal source/custody was not retained")?;
    assert!(unwound);
    assert!(clock.native_calls.load(Ordering::SeqCst) > 0);
    assert!(!super::super::fixture::native_probe::is_current(clock.as_ref()
        as *const RetirementClock
        as usize));
    assert_eq!(clock.contended.load(Ordering::SeqCst), 0);
    let snapshot = storage.snapshot();
    assert!(!snapshot.retirement_in_progress());
    assert_eq!(snapshot.live_handles(), if cleanup { before } else { before + 1 });
    assert_eq!(snapshot.reads(), 1);
    assert!(snapshot.admission_closed());
    assert_eq!(snapshot.cleanup_deadline(), cleanup.then_some(Moment::from_nanos(100)));
    if cleanup {
        assert!(before > 0);
    }
    assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
    // Physical fake completion is separate from the now-invalid Storage control.
    filesystem.complete_external_closes(&context)?;
    assert_eq!(storage.snapshot().live_handles(), 0);
    assert_eq!(storage.snapshot().reads(), 1);
    drop((storage, operation, filesystem, context));
    drop(completion);
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn retirement_provider_unwind_restores_whole_native_capsule_and_original_claim() -> TestResult {
    provider_unwind_run(true)
}
#[test]
fn normal_native_provider_unwind_restores_partial_output_and_original_capsule() -> TestResult {
    provider_unwind_run(false)
}

#[test]
fn detached_port_clock_observation_outside_native_callback_cannot_consume_armed_failure()
-> TestResult {
    retirement_job(|context, clock| {
        let filesystem = Filesystem::new();
        filesystem.probe_native_context(clock.clone());
        let (mut storage, operation) = setup(&context, filesystem)?;
        *clock.target.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(storage.shared.clone());
        let target = ClockTarget(clock.clone());
        clock.panic_detached.store(true, Ordering::SeqCst);
        let mut access = StateAccess::acquire(&storage.shared, operation.key());
        access.check(operation.key(), &context, false)?;
        access.call_native(|port| {
            assert!(storage.shared.inner.try_lock().is_ok_and(|state| state.port.is_none()));
            assert_eq!(Clock::now(clock.as_ref()), Moment::from_nanos(17));
            assert!(clock.panic_detached.load(Ordering::SeqCst));
            assert_eq!(clock.native_calls.load(Ordering::SeqCst), 0);
            assert_eq!(port.live_handles(), 0);
        })?;
        drop(access);
        clock.panic_detached.store(false, Ordering::SeqCst);
        storage.retire(&operation, &context)?;
        assert!(clock.native_calls.load(Ordering::SeqCst) > 0);
        assert_eq!(clock.contended.load(Ordering::SeqCst), 0);
        drop(target);
        drop((storage, operation));
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}
