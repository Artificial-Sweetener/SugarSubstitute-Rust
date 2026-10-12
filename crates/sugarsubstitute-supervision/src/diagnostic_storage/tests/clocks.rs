// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify supplied clock failures retain custody instead of inventing elapsed time.
use super::*;
#[test]
fn diagnostic_failed_cleanup_retains_original_scope_and_rejects_deadline_renewal() -> TestResult {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(TestClock(AtomicU64::new(17)));
    let owner = ExecutionOwner::new(clock, wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let retained = Arc::new(Mutex::new(None));
    for (id, deadline) in [(1, 100), (2, 101), (3, 100)] {
        let state = retained.clone();
        let mut job = client.submit_blocking(
            &scope,
            JobSpec {
                id: JobId::new(id)?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(Moment::from_nanos(deadline)),
                reservation_bytes: if id == 1 { DIAGNOSTIC_STORAGE_BYTES } else { 0 },
                replacement: None,
            },
            move |context| -> TestResult {
                if id == 1 {
                    let filesystem = Filesystem::new();
                    let object = DiagnosticObject::Summary(incident()?);
                    filesystem.put(object, b"summary");
                    let (mut storage, operation) = setup(&context, filesystem.clone())?;
                    drop(storage.read(object, 64, &operation, &context)?);
                    filesystem
                        .0
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .faults
                        .close = true;
                    assert_eq!(
                        storage
                            .retire(&operation, &context)
                            .err()
                            .ok_or("failed cleanup succeeded")?
                            .code(),
                        Code::Retirement
                    );
                    assert_eq!(
                        storage.snapshot().cleanup_deadline(),
                        Some(Moment::from_nanos(100))
                    );
                    *state.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                        Some((storage, operation, filesystem));
                } else {
                    let (mut storage, operation, filesystem) = state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .take()
                        .ok_or("retained cleanup owner missing")?;
                    filesystem
                        .0
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .faults
                        .close = false;
                    if id == 2 {
                        assert_eq!(
                            storage
                                .retire(&operation, &context)
                                .err()
                                .ok_or("cleanup renewed")?
                                .code(),
                            Code::Deadline
                        );
                        assert_eq!(storage.snapshot().live_handles(), 1);
                        assert_eq!(
                            storage.snapshot().cleanup_deadline(),
                            Some(Moment::from_nanos(100))
                        );
                        *state.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                            Some((storage, operation, filesystem));
                    } else {
                        storage.retire(&operation, &context)?;
                        assert_eq!(storage.snapshot().live_handles(), 0);
                    }
                }
                Ok(())
            },
        )?;
        assert!(matches!(observe_job(&client, &wake, &mut job)?, JobOutcome::Success(())));
        drop(job);
        assert_eq!(
            scope.snapshot().reserved_bytes,
            if id < 3 { DIAGNOSTIC_STORAGE_BYTES } else { 0 }
        );
    }
    drop(owner);
    Ok(())
}
#[test]
fn diagnostic_post_open_deadline_preserves_actual_retained_handle() -> TestResult {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(TestClock(AtomicU64::new(17)));
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let retained = Arc::new(Mutex::new(None));
    let output = retained.clone();
    let job_clock = clock.clone();
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
            let clock = job_clock;
            let filesystem = Filesystem::new();
            let object = DiagnosticObject::Summary(incident()?);
            filesystem.put(object, b"summary");
            let (mut storage, operation) = setup(&context, filesystem.clone())?;
            {
                let mut disk =
                    filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                disk.after_open = Some(clock.clone());
                disk.after_open_time = 100;
            }
            assert_eq!(
                storage
                    .read(object, 64, &operation, &context)
                    .err()
                    .ok_or("expired read admitted")?
                    .code(),
                Code::Deadline
            );
            assert_eq!(storage.snapshot().live_handles(), 1);
            assert_eq!(storage.snapshot().reads(), 1);
            assert_eq!(
                storage
                    .shared
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .operation
                    .as_ref()
                    .ok_or("operation missing")?
                    .last_observed,
                Moment::from_nanos(100)
            );
            assert_eq!(
                storage
                    .retire(&operation, &context)
                    .err()
                    .ok_or("expired cleanup admitted")?
                    .code(),
                Code::Deadline
            );
            *output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some((storage, operation));
            Ok(())
        },
    )?;
    assert!(matches!(observe_job(&client, &wake, &mut job)?, JobOutcome::TimedOut));
    drop(job);
    assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
    let cleanup = client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let input = retained.clone();
    let mut cleanup_job = client.submit_blocking(
        &cleanup,
        JobSpec {
            id: JobId::new(2)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(200)),
            reservation_bytes: 0,
            replacement: None,
        },
        move |context| -> TestResult {
            let (mut storage, operation) = input
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .ok_or("retained owner missing")?;
            storage.retire(&operation, &context)?;
            assert_eq!(storage.snapshot().live_handles(), 0);
            assert_eq!(storage.snapshot().cleanup_deadline(), Some(Moment::from_nanos(200)));
            assert_eq!(storage.snapshot().deadline(), Some(Moment::from_nanos(100)));
            assert_eq!(context.now(), Moment::from_nanos(100));
            Ok(())
        },
    )?;
    assert!(matches!(observe_job(&client, &wake, &mut cleanup_job)?, JobOutcome::Success(())));
    drop(cleanup_job);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    drop(owner);
    Ok(())
}
#[test]
fn diagnostic_clock_regression_closes_admission_with_actual_custody_retained() -> TestResult {
    use std::error::Error;
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(TestClock(AtomicU64::new(17)));
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let retained = Arc::new(Mutex::new(None));
    let output = retained.clone();
    let job_clock = clock.clone();
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
            let object = DiagnosticObject::Summary(incident()?);
            filesystem.put(object, b"summary");
            let (storage, operation) = setup(&context, filesystem.clone())?;
            {
                let mut disk =
                    filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                disk.after_open = Some(job_clock);
                disk.after_open_time = 16;
            }
            // Preserve the real owner before the guarded post-open clock can unwind.
            *output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some((storage, operation, filesystem, context));
            let mut slot = output.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let (storage, operation, _, context) = slot.as_mut().ok_or("retained owner missing")?;
            storage.read(object, 64, operation, context)?;
            Err("Regressing clock admitted a read".into())
        },
    )?;
    let completion = loop {
        let old = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        let (count, timeout) = wake
            .changed
            .wait_timeout_while(
                wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                Duration::from_secs(10),
                |count| *count == old,
            )
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && *count == old {
            return Err("Fatal clock completion stalled".into());
        }
    };
    assert!(matches!(completion.outcome, JobOutcome::Panicked(_)), "{completion:?}");
    let failure = client.now().err().ok_or("Fatal clock returned a timestamp")?;
    assert_eq!(failure.operation(), Operation::Infrastructure);
    assert!(
        failure
            .source()
            .is_some_and(|cause| cause.to_string().contains("Monotonic interval regressed"))
    );
    assert!(matches!(
        client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None),
        Err(AdmissionError::ShuttingDown)
    ));
    assert_eq!(clock.0.load(Ordering::SeqCst), 16);
    let (storage, operation, mut filesystem, context) = retained
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .ok_or("Fatal clock lost the actual storage owner")?;
    let snapshot = storage.snapshot();
    assert_eq!(snapshot.deadline(), Some(Moment::from_nanos(100)));
    assert_eq!(snapshot.cleanup_deadline(), None);
    assert_eq!(snapshot.live_handles(), 1);
    assert_eq!(snapshot.reads(), 1);
    assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
    assert_eq!(
        storage
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .operation
            .as_ref()
            .ok_or("operation missing")?
            .last_observed,
        Moment::from_nanos(17)
    );
    // Observe only actual external fixture closure; the public storage owner has
    // no valid clock with which to acknowledge retirement or release its slot.
    Port::retire(&mut filesystem, &context)?;
    assert_eq!(Port::live_handles(&filesystem), 0);
    assert_eq!(storage.snapshot().reads(), 1);
    assert!(storage.snapshot().writer_held());
    drop((storage, operation, filesystem, context));
    drop(completion);
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(clock.0.load(Ordering::SeqCst), 16);
    Ok(())
}
