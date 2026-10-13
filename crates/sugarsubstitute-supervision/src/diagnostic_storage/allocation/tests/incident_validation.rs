// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify fixed callback control admission and final original claim lifetime.
use super::*;

#[test]
fn incident_validation_controls_are_one_issue_and_retain_original_claim_until_both_drop()
-> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = setup(&context, Filesystem::new())?;
        let mut decode = storage.recovery_decode(&operation, &context)?;
        let controls = decode.incident_validation_controls()?;
        assert_eq!(
            decode
                .incident_validation_controls()
                .err()
                .ok_or("Second validation controls admitted")?
                .code(),
            Code::Capacity
        );
        let (inner, outer) = controls.into_parts();
        drop(decode);
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        assert!(context.memory().claim(1).is_err());
        drop(inner);
        assert!(context.memory().claim(1).is_err());
        drop(outer);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn expired_original_decode_rejects_incident_controls_before_cause_construction() -> TestResult {
    admitted_timeout(|context, clock| {
        let memory = context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?;
        let mut storage = DiagnosticStorage::from_port(
            allocation::owner(memory)?,
            OwnedPort::new(Box::new(Filesystem::new()), None),
        )?;
        let operation = storage.begin_operation(key(1)?, &context)?;
        let mut decode = storage.recovery_decode(&operation, &context)?;
        clock.0.store(100, Ordering::SeqCst);
        let failure = decode
            .incident_validation_controls()
            .err()
            .ok_or("Expired original validation controls admitted")?;
        assert_eq!(failure.operation(), Some(operation.key()));
        assert_eq!(failure.code(), Code::Deadline);
        let cause = DiagnosticRecoveryValidationError::incident_allocation_rejected(failure);
        let original = std::error::Error::source(&cause)
            .and_then(|source| source.downcast_ref::<DiagnosticStorageError>())
            .ok_or("Original inline admission failure replaced")?;
        assert_eq!(original.code(), Code::Deadline);
        assert_eq!(storage.snapshot().live_handles(), 0);
        Ok(())
    })
}
