// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise cumulative admission and actual original claim lifetimes.
use super::*;
use std::io::Write;
#[path = "tests/incident_validation.rs"]
mod incident_validation;
#[path = "tests/inventory_cardinality.rs"]
mod inventory_cardinality;
#[path = "tests/native_owner.rs"]
mod native_owner;
#[path = "tests/native_retention.rs"]
mod native_retention;
#[path = "tests/reservation.rs"]
mod reservation;

#[test]
fn rejected_callback_retains_primary_and_exact_post_control_failure() -> TestResult {
    admitted_timeout(|context, clock| {
        let memory = context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?;
        let mut storage = DiagnosticStorage::from_port(
            allocation::owner(memory)?,
            OwnedPort::new(Box::new(Filesystem::new()), None),
        )?;
        let operation = storage.begin_operation(key(1)?, &context)?;
        let cause = allocation::decode_callback::<()>(&mut storage, &operation, &context, |_| {
            clock.0.store(100, Ordering::SeqCst);
            Err(DiagnosticRecoveryValidationError::caused(
                DiagnosticRecoveryValidationCode::InvalidSchema,
                std::io::Error::from(std::io::ErrorKind::PermissionDenied),
            ))
        })
        .err()
        .ok_or("Rejected codec returned a value")?;
        assert_eq!(cause.code(), Code::InvalidIdentity);
        assert_eq!(cause.post_control_failure(), Some(Code::Deadline));
        let native = std::error::Error::source(&cause).ok_or("Missing native wrapper")?;
        let validation = std::error::Error::source(native)
            .and_then(|source| source.downcast_ref::<DiagnosticRecoveryValidationError>())
            .ok_or("Genuine validation error replaced")?;
        assert_eq!(validation.code(), DiagnosticRecoveryValidationCode::InvalidSchema);
        let original = std::error::Error::source(validation)
            .and_then(|source| source.downcast_ref::<std::io::Error>())
            .ok_or("Original cause replaced")?;
        assert_eq!(original.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(storage.snapshot().live_handles(), 0);
        assert_eq!(
            storage
                .retire(&operation, &context)
                .err()
                .ok_or("Expired normal context retired custody")?
                .code(),
            Code::Deadline,
        );
        Ok(())
    })
}

#[test]
fn collection_transfer_is_one_use_and_escaped_record_keeps_original_claim() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem)?;
        let mut collection = storage.recovery_collection(&operation, &context)?;
        assert_eq!(
            storage
                .recovery_collection(&operation, &context)
                .err()
                .ok_or("Second collection admitted")?
                .code(),
            Code::Capacity
        );
        let record = collection.transfer_record(0, &context)?;
        assert_eq!(
            collection.transfer_record(0, &context).err().ok_or("Record index reused")?.code(),
            Code::InvalidIdentity
        );
        assert_eq!(
            collection
                .transfer_record(128, &context)
                .err()
                .ok_or("Unbounded index admitted")?
                .code(),
            Code::Capacity
        );
        drop(collection);
        let successor = storage.recovery_collection(&operation, &context)?;
        drop(successor);
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        assert!(context.memory().claim(1).is_err());
        drop(record);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn retained_error_loan_keeps_decode_exclusive_until_actual_drop() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem)?;
        let mut decode = storage.recovery_decode(&operation, &context)?;
        let error = decode.retain_error()?;
        assert_eq!(
            decode.retain_error().err().ok_or("Error owner duplicated")?.code(),
            Code::Capacity
        );
        drop(decode);
        assert_eq!(
            storage
                .recovery_decode(&operation, &context)
                .err()
                .ok_or("Live error phase reused")?
                .code(),
            Code::Capacity
        );
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        assert!(context.memory().claim(1).is_err());
        drop(error);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn canonical_buffer_rejects_growth_and_retains_original_after_retirement() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem)?;
        let mut buffer = storage.record_buffer(&operation, &context)?;
        buffer.write_all(&[3; DIAGNOSTIC_IO_BYTES])?;
        assert_eq!(
            buffer.write(&[4]).err().ok_or("Buffer grew")?.kind(),
            std::io::ErrorKind::OutOfMemory
        );
        assert_eq!(buffer.as_bytes(), &[3; DIAGNOSTIC_IO_BYTES]);
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        assert!(context.memory().claim(1).is_err());
        drop(buffer);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn repeated_retained_inventories_saturate_before_scan_and_resume_after_drop() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem)?;
        let mut held = Vec::new();
        loop {
            let before = lock(&storage.shared).scan_generation;
            match storage.inventory(&operation, &context) {
                Ok(inventory) => held.push(inventory),
                Err(cause) => {
                    assert_eq!(cause.code(), Code::Capacity);
                    assert_eq!(lock(&storage.shared).scan_generation, before);
                    break;
                }
            }
        }
        assert!(!held.is_empty());
        drop(held);
        let next = storage.inventory(&operation, &context)?;
        assert_eq!(next.status(), DiagnosticInventoryStatus::Complete);
        drop(next);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}

#[test]
fn escaped_identity_aliases_keep_one_original_control_after_owner_retirement() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem)?;
        let identity = storage.recovery_publication_identity(&operation, &context)?;
        let first = identity.retain_for_token(&operation, &context)?;
        let second = first.retain_for_token(&operation, &context)?;
        assert!(identity.matches(&first));
        assert!(first.matches(&second));
        storage.retire(&operation, &context)?;
        assert_eq!(
            second
                .retain_for_token(&operation, &context)
                .err()
                .ok_or("Retired correlation admitted alias")?
                .code(),
            Code::InvalidIdentity
        );
        drop(operation);
        drop(storage);
        drop(identity);
        drop(first);
        assert!(context.memory().claim(1).is_err());
        drop(second);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

fn admitted_timeout(
    work: impl FnOnce(JobContext, Arc<TestClock>) -> TestResult + Send + 'static,
) -> TestResult {
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
        move |context| {
            *output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some(work(context, clock));
            Ok::<(), TestError>(())
        },
    )?;
    assert!(matches!(observe_job(&client, &wake, &mut job)?, JobOutcome::TimedOut));
    let checked = retained
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .ok_or("Expired callback evidence missing")?;
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    checked
}

#[test]
fn collection_error_transfer_keeps_phase_and_original_claim_until_cause_drops() -> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = setup(&context, Filesystem::new())?;
        let collection = storage.recovery_collection(&operation, &context)?;
        let cause = Vec::<u8>::new()
            .try_reserve_exact(usize::MAX)
            .err()
            .ok_or("Capacity overflow succeeded")?;
        let error = (cause, collection.into_error());
        assert_eq!(
            storage
                .recovery_collection(&operation, &context)
                .err()
                .ok_or("Error released collection")?
                .code(),
            Code::Capacity
        );
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        assert!(context.memory().claim(1).is_err());
        drop(error);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[path = "tests/storage_metadata.rs"]
mod storage_metadata;
