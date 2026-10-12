// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the first execution context authoritative for recovered-victim observations.
use super::*;

#[test]
fn diagnostic_recovered_victim_observation_rejects_foreign_expired_and_original_cancelled_jobs()
-> TestResult {
    for mode in 0..3 {
        let wake = Arc::new(Wake::default());
        let clock = Arc::new(TestClock(AtomicU64::new(17)));
        let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let retained = Arc::new(Mutex::new(None));
        let output = retained.clone();
        let gate = Arc::new((Mutex::new((false, false)), Condvar::new()));
        let worker_gate = gate.clone();
        let mut first = client.submit_blocking(
            &scope,
            spec(1, DIAGNOSTIC_STORAGE_BYTES, 100)?,
            move |context| -> TestResult {
                let filesystem = Filesystem::new();
                let (mut storage, operation) = setup(&context, filesystem.clone())?;
                let plan = recovery_plan(operation.key())?;
                let (mut journal, mut ledger) =
                    reads(&mut storage, &filesystem, &operation, plan, ORIGINAL, &context)?;
                let permit = storage.recover_disposition(
                    &mut journal,
                    &mut ledger,
                    &validator(plan, DiagnosticRecoveryLedgerCase::Original),
                    &operation,
                    &context,
                )?;
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
                    return Err("victim observation handshake stalled".into());
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
            return Err("victim admission handshake stalled".into());
        }
        if mode == 2 {
            first.cancel();
        }
        state.1 = true;
        changed.notify_all();
        drop(state);
        let outcome = observe_job(&client, &wake, &mut first)?;
        if mode == 2 {
            assert!(matches!(outcome, JobOutcome::Cancelled));
        } else {
            assert!(matches!(outcome, JobOutcome::Success(())));
        }
        if mode == 1 {
            clock.0.store(100, Ordering::SeqCst);
        }
        let foreign =
            client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
        let second_scope = if mode == 0 { &foreign } else { &scope };
        let mut second = client.submit_blocking(
            second_scope,
            spec(2, 0, if mode == 1 { 200 } else { 100 })?,
            move |context| -> TestResult {
                let (mut storage, operation, mut permit, filesystem) = retained
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take()
                    .ok_or("retained victim intent missing")?;
                let expected = match mode {
                    0 => Code::InvalidIdentity,
                    1 => Code::Deadline,
                    _ => Code::Cancelled,
                };
                assert_eq!(
                    storage
                        .recovered_victim_state(&mut permit, &operation, &context)
                        .err()
                        .ok_or("invalid context reported settled")?
                        .code(),
                    expected
                );
                assert_eq!(storage.snapshot().live_handles(), 1);
                assert!(storage.snapshot().disposition_retained());
                assert_eq!(operation.original_deadline(), Moment::from_nanos(100));
                assert_eq!(
                    filesystem
                        .0
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .mutations,
                    0
                );
                storage.retire(&operation, &context)?;
                assert_eq!(storage.snapshot().live_handles(), 0);
                Ok(())
            },
        )?;
        assert!(matches!(observe_job(&client, &wake, &mut second)?, JobOutcome::Success(())));
        drop((first, second));
        drop(owner);
        assert_eq!(scope.snapshot().reserved_bytes, 0);
    }
    Ok(())
}
fn spec(id: u64, memory: u64, deadline: u64) -> Result<JobSpec, TestError> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(deadline)),
        reservation_bytes: memory,
        replacement: None,
    })
}
