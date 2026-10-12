// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe the original payload job cancellation through the actual execution owner.
use super::*;
#[test]
fn diagnostic_payload_discard_same_scope_new_job_cannot_mask_original_cancellation() -> TestResult {
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
            let filesystem = disk()?;
            let (mut storage, operation) = setup(&context, filesystem.clone())?;
            let permit = normal(&mut storage, &operation, &context)?;
            *output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some((storage, operation, permit, filesystem));
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
        return Err("payload admission handshake stalled".into());
    }
    first.cancel();
    state.1 = true;
    changed.notify_all();
    drop(state);
    assert!(matches!(observe_job(&client, &wake, &mut first)?, JobOutcome::Cancelled));
    let input = retained.clone();
    let mut second = client.submit_blocking(&scope, spec(2, 0)?, move |context| -> TestResult {
        assert!(!context.cancellation_requested());
        let (mut storage, operation, mut permit, filesystem) = input
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .ok_or("original owner missing")?;
        let mutations = filesystem.snapshot().mutation_count();
        assert_eq!(
            storage
                .complete_payload_discard(&mut permit, &operation, &context)
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
