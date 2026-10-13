// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain payload custody across callback panic and globally fatal execution clocks.
use super::*;
use crate::diagnostic_storage::fixture::BoundaryClock;
#[test]
fn diagnostic_payload_discard_callback_panic_and_global_clock_failure_preserve_custody()
-> TestResult {
    use std::error::Error;
    for fatal in [false, true] {
        let wake = Arc::new(Wake::default());
        let clock = Arc::new(TestClock(AtomicU64::new(17)));
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
                let filesystem = disk()?;
                let (mut storage, operation) = setup(&context, filesystem.clone())?;
                let (preparation, validator, original, candidate) =
                    prepare(&mut storage, &operation, &context)?;
                *validator.hook.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some(Box::new(move || {
                        if fatal {
                            callback_clock.observe(16);
                        } else {
                            std::panic::resume_unwind(Box::new("injected semantic callback panic"));
                        }
                    }));
                *output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((
                    storage,
                    operation,
                    preparation,
                    validator,
                    filesystem,
                    context,
                    original,
                    candidate,
                ));
                let mut slot = output.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                let (storage, operation, preparation, validator, _, context, _, _) =
                    slot.as_mut().ok_or("retained payload missing")?;
                storage.seal_payload_discard(
                    preparation,
                    JOURNAL,
                    validator,
                    operation,
                    context,
                )?;
                Err("failed callback published payload".into())
            },
        )?;
        let completion = without_clock(&wake, &mut job)?;
        assert!(matches!(completion.outcome, JobOutcome::Panicked(_)), "{completion:?}");
        assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
        {
            let slot = retained.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let (storage, _, _, _, filesystem, _, _, _) =
                slot.as_ref().ok_or("lost retained payload")?;
            assert!(storage.snapshot().disposition_retained());
            assert_eq!(storage.snapshot().stages(), 1);
            assert_eq!(storage.snapshot().reads(), 1);
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
            let (
                storage,
                operation,
                preparation,
                validator,
                mut filesystem,
                context,
                original,
                candidate,
            ) = retained
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .ok_or("fatal owner missing")?;
            // Only the primitive external fixture is retired; no public storage settlement is claimed.
            filesystem.complete_external_closes(&context)?;
            assert_eq!(filesystem.snapshot().live_outputs(), 0);
            assert!(storage.snapshot().disposition_retained());
            assert_eq!(storage.snapshot().stages(), 1);
            drop((
                storage,
                operation,
                preparation,
                validator,
                filesystem,
                context,
                original,
                candidate,
            ));
            assert_eq!(clock.0.load(Ordering::SeqCst), 16);
        } else {
            let cleanup =
                client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
            let input = retained.clone();
            let mut close =
                client.submit_blocking(&cleanup, spec(2, 0)?, move |context| -> TestResult {
                    let (
                        mut storage,
                        operation,
                        preparation,
                        validator,
                        filesystem,
                        first,
                        original,
                        candidate,
                    ) = input
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .take()
                        .ok_or("panic owner missing")?;
                    storage.retire(&operation, &context)?;
                    assert_eq!(filesystem.snapshot().live_outputs(), 0);
                    drop((
                        storage,
                        operation,
                        preparation,
                        validator,
                        filesystem,
                        first,
                        original,
                        candidate,
                    ));
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
