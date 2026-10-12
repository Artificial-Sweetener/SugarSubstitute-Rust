// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual post-open expiry without rewinding or renewing recovery admission.
use super::*;
#[test]
fn diagnostic_recovery_stage_post_open_expiry_retains_actual_outputs_for_original_cleanup()
-> TestResult {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(TestClock(AtomicU64::new(17)));
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let retained = Arc::new(Mutex::new(None));
    let output = retained.clone();
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
            let (mut storage, operation, mut permit, _, _, _) =
                super::stage::prepared(&context, &filesystem, false)?;
            {
                let mut disk =
                    filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                disk.after_open = Some(clock);
                disk.after_open_time = 100;
            }
            assert_eq!(
                storage
                    .recover_completion_stage(&mut permit, &operation, &context)
                    .err()
                    .ok_or("expired stage admitted")?
                    .code(),
                Code::Deadline
            );
            assert_eq!(storage.snapshot().live_handles(), 2);
            assert_eq!(storage.snapshot().stages(), 1);
            assert_eq!(operation.original_deadline(), Moment::from_nanos(100));
            drop(permit);
            *output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some((storage, operation));
            Ok(())
        },
    )?;
    assert!(matches!(observe_job(&client, &wake, &mut job)?, JobOutcome::TimedOut));
    drop(job);
    assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
    let cleanup = client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
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
            let (mut storage, operation) = retained
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .ok_or("owner missing")?;
            storage.retire(&operation, &context)?;
            assert_eq!(storage.snapshot().live_handles(), 0);
            assert_eq!(storage.snapshot().deadline(), Some(Moment::from_nanos(100)));
            assert_eq!(storage.snapshot().cleanup_deadline(), Some(Moment::from_nanos(200)));
            assert_eq!(context.now(), Moment::from_nanos(100));
            Ok(())
        },
    )?;
    assert!(matches!(observe_job(&client, &wake, &mut job)?, JobOutcome::Success(())));
    drop(job);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    drop(owner);
    Ok(())
}
