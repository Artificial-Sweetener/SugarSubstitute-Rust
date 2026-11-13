// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A semantic callback cannot extend the first publication deadline across native effects.
use super::*;
#[test]
fn diagnostic_migration_callback_expiry_fences_activation_and_retains_original_cleanup_custody()
-> TestResult {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(TestClock(AtomicU64::new(17)));
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let retained = Arc::new(Mutex::new(None));
    let output = retained.clone();
    let mut first = client.submit_blocking(
        &scope,
        spec(1, DIAGNOSTIC_STORAGE_BYTES, 100)?,
        move |context| -> TestResult {
            let filesystem = Filesystem::new();
            filesystem.put(DiagnosticObject::Summary(incident()?), LEGACY);
            let (mut storage, operation) = setup(&context, filesystem.clone())?;
            let (mut permit, validator) = normal(&mut storage, &operation, &context)?;
            let mutations = filesystem.snapshot().mutation_count();
            *validator.hook.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some(Box::new(move || clock.0.store(100, Ordering::SeqCst)));
            assert_eq!(
                storage
                    .complete_summary_migration(&mut permit, &validator, &operation, &context)
                    .err()
                    .ok_or("expired callback admitted rename")?
                    .code(),
                Code::Deadline
            );
            assert_eq!(filesystem.snapshot().mutation_count(), mutations);
            assert_eq!(storage.snapshot().live_handles(), 4);
            assert_eq!(storage.snapshot().deadline(), Some(Moment::from_nanos(100)));
            assert_eq!(context.now(), Moment::from_nanos(100));
            *output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some((storage, operation, permit, validator, filesystem));
            Ok(())
        },
    )?;
    assert!(matches!(observe_job(&client, &wake, &mut first)?, JobOutcome::TimedOut));
    drop(first);
    assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
    let cleanup = client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let mut close =
        client.submit_blocking(&cleanup, spec(2, 0, 200)?, move |context| -> TestResult {
            let (mut storage, operation, permit, validator, filesystem) = retained
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .ok_or("expired migration custody missing")?;
            let mutations = filesystem.snapshot().mutation_count();
            storage.retire(&operation, &context)?;
            assert_eq!(storage.snapshot().deadline(), Some(Moment::from_nanos(100)));
            assert_eq!(storage.snapshot().cleanup_deadline(), Some(Moment::from_nanos(200)));
            assert_eq!(filesystem.snapshot().live_outputs(), 0);
            assert_eq!(filesystem.snapshot().mutation_count(), mutations);
            assert_eq!(context.now(), Moment::from_nanos(100));
            drop((storage, operation, permit, validator, filesystem));
            Ok(())
        })?;
    assert!(matches!(observe_job(&client, &wake, &mut close)?, JobOutcome::Success(())));
    drop(close);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
fn spec(id: u64, bytes: u64, deadline: u64) -> Result<JobSpec, TestError> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(deadline)),
        reservation_bytes: bytes,
        replacement: None,
    })
}
