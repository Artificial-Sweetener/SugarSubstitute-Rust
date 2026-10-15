// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe original job cancellation and globally fatal clocks through the actual execution owner.
use super::*;
use sugarsubstitute_execution::clock_fixture::CallbackRegressionClock;
#[test]
fn diagnostic_migration_same_scope_new_job_cannot_mask_original_cancellation() -> TestResult {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(TestClock(AtomicU64::new(17)));
    let owner = ExecutionOwner::new(clock, wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let retained = Arc::new(Mutex::new(None));
    let output = retained.clone();
    let gate = Arc::new((Mutex::new((false, false)), Condvar::new()));
    let worker_gate = gate.clone();
    let mut first = client.submit_blocking(
        &scope,
        spec(1, DIAGNOSTIC_STORAGE_BYTES)?,
        move |context| -> TestResult {
            let filesystem = Filesystem::new();
            filesystem.put(DiagnosticObject::Summary(incident()?), LEGACY);
            let (mut storage, operation) = setup(&context, filesystem.clone())?;
            let (permit, validator) = normal(&mut storage, &operation, &context)?;
            *output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some((storage, operation, permit, validator, filesystem));
            let (mutex, changed) = &*worker_gate;
            let mut state = mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            state.0 = true;
            changed.notify_all();
            let (_state, timeout) = changed
                .wait_timeout_while(state, Duration::from_secs(10), |state| !state.1)
                .map_err(|cause| std::io::Error::other(cause.to_string()))?;
            if timeout.timed_out() {
                return Err("cancellation handshake stalled".into());
            }
            Ok(())
        },
    )?;
    let (mutex, changed) = &*gate;
    let (mut state, timeout) = changed
        .wait_timeout_while(
            mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
            Duration::from_secs(10),
            |state| !state.0,
        )
        .map_err(|cause| std::io::Error::other(cause.to_string()))?;
    if timeout.timed_out() {
        return Err("migration admission handshake stalled".into());
    }
    first.cancel();
    state.1 = true;
    changed.notify_all();
    drop(state);
    assert!(matches!(observe_job(&client, &wake, &mut first)?, JobOutcome::Cancelled));
    let input = retained.clone();
    let mut second = client.submit_blocking(&scope, spec(2, 0)?, move |context| -> TestResult {
        assert!(!context.cancellation_requested());
        let (mut storage, operation, mut permit, validator, filesystem) = input
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .ok_or("original owner missing")?;
        let mutations = filesystem.snapshot().mutation_count();
        assert_eq!(
            storage
                .complete_summary_migration(&mut permit, &validator, &operation, &context)
                .err()
                .ok_or("new job concealed original cancellation")?
                .code(),
            Code::Cancelled
        );
        assert!(storage.snapshot().disposition_retained());
        assert_eq!(filesystem.snapshot().mutation_count(), mutations);
        assert_eq!(storage.snapshot().deadline(), Some(Moment::from_nanos(100)));
        storage.retire(&operation, &context)?;
        assert_eq!(filesystem.snapshot().live_outputs(), 0);
        assert_eq!(filesystem.snapshot().mutation_count(), mutations);
        Ok(())
    })?;
    assert!(matches!(observe_job(&client, &wake, &mut second)?, JobOutcome::Success(())));
    drop((first, second));
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn diagnostic_migration_callback_panic_and_global_clock_failure_preserve_actual_custody()
-> TestResult {
    use std::error::Error;
    for fatal in [false, true] {
        let wake = Arc::new(Wake::default());
        let clock = CallbackRegressionClock::new(Moment::from_nanos(17));
        let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let retained = Arc::new(Mutex::new(None));
        let output = retained.clone();
        let callback_clock = clock.clone();
        let mut job = client.submit_blocking(
            &scope,
            spec(1, DIAGNOSTIC_STORAGE_BYTES)?,
            move |context| -> TestResult {
                let filesystem = Filesystem::new();
                filesystem.put(DiagnosticObject::Summary(incident()?), LEGACY);
                let (mut storage, operation) = setup(&context, filesystem.clone())?;
                let (permit, validator) = normal(&mut storage, &operation, &context)?;
                *validator.hook.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some(Box::new(move || {
                        if fatal {
                            // Only this callback's next real clock observation regresses;
                            // execution guardians retain the healthy epoch until it is consumed.
                            assert!(callback_clock.arm_current_callback(Moment::from_nanos(16)));
                        } else {
                            std::panic::resume_unwind(Box::new("injected semantic callback panic"));
                        }
                    }));
                *output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some((storage, operation, permit, validator, filesystem, context));
                let mut slot = output.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                let (storage, operation, permit, validator, _, context) =
                    slot.as_mut().ok_or("retained migration missing")?;
                storage.complete_summary_migration(permit, validator, operation, context)?;
                Err("failed callback published migration".into())
            },
        )?;
        let completion = without_clock(&wake, &mut job)?;
        assert!(matches!(completion.outcome, JobOutcome::Panicked(_)), "{completion:?}");
        assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
        {
            let slot = retained.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let (storage, _, _, _, filesystem, _) =
                slot.as_ref().ok_or("lost retained migration")?;
            assert!(storage.snapshot().disposition_retained());
            assert_eq!(storage.snapshot().stages(), 2);
            assert_eq!(storage.snapshot().reads(), 2);
            assert_eq!(filesystem.snapshot().live_outputs(), 4);
            assert_eq!(storage.snapshot().deadline(), Some(Moment::from_nanos(100)));
            assert_eq!(
                lock(&storage.shared).operation.as_ref().ok_or("binding missing")?.last_observed,
                Moment::from_nanos(17)
            );
        }
        if fatal {
            let cause = client.now().err().ok_or("global fatal clock produced time")?;
            assert_eq!(cause.operation(), Operation::Infrastructure);
            assert!(
                cause.source().is_some_and(|source| source
                    .to_string()
                    .contains("Monotonic interval regressed"))
            );
            assert!(matches!(
                client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None),
                Err(AdmissionError::ShuttingDown)
            ));
            let (storage, operation, permit, validator, mut filesystem, context) = retained
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .ok_or("fatal owner missing")?;
            // Only the primitive external fixture is retired; no public storage settlement is claimed.
            filesystem.complete_external_closes(&context)?;
            assert_eq!(filesystem.snapshot().live_outputs(), 0);
            assert!(storage.snapshot().disposition_retained());
            assert_eq!(storage.snapshot().stages(), 2);
            drop((storage, operation, permit, validator, filesystem, context));
            assert_eq!(clock.requested_regression(), Some(Moment::from_nanos(16)));
            assert_eq!(clock.regressed_samples(), 1);
        } else {
            let cleanup =
                client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
            let input = retained.clone();
            let mut close =
                client.submit_blocking(&cleanup, spec(2, 0)?, move |context| -> TestResult {
                    let (mut storage, operation, permit, validator, filesystem, first) = input
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .take()
                        .ok_or("panic owner missing")?;
                    storage.retire(&operation, &context)?;
                    assert_eq!(filesystem.snapshot().live_outputs(), 0);
                    drop((storage, operation, permit, validator, filesystem, first));
                    Ok(())
                })?;
            assert!(matches!(observe_job(&client, &wake, &mut close)?, JobOutcome::Success(())));
            drop(close);
        }
        drop(completion);
        drop(job);
        drop(owner);
        assert_eq!(scope.snapshot().reserved_bytes, 0);
    }
    Ok(())
}
fn spec(id: u64, bytes: u64) -> Result<JobSpec, TestError> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(100)),
        reservation_bytes: bytes,
        replacement: None,
    })
}
fn without_clock(
    wake: &Wake,
    job: &mut OwnedJob<(), TestError>,
) -> Result<JobCompletion<(), TestError>, TestError> {
    loop {
        let old = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(completion) = job.try_take_completion() {
            return Ok(completion);
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
            return Err("failed callback completion stalled".into());
        }
    }
}
