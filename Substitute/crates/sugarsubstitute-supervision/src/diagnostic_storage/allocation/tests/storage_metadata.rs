// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve independent storage and cause metadata across actual private aliases.
use super::*;
use crate::claim_error::{BoundedNativeCause, NativeCauseDestination};
use std::error::Error;

#[test]
fn storage_error_preserves_distinct_operation_keys_and_inline_cause_metadata() -> TestResult {
    let original = DiagnosticNativeAllocationFailure::rejected(Some(key(2)?), Code::Capacity);
    let cause = DiagnosticStorageError::native(
        Some(key(1)?),
        Code::Read,
        ClaimError::diagnostic_allocation(original),
    );
    let alias = cause.clone();
    let cause =
        cause.with_post_control(Some(DiagnosticStorageError::rejected(None, Code::Cancelled)));
    assert_eq!(cause.operation(), Some(key(1)?));
    assert_eq!(cause.code(), Code::Read);
    assert_eq!(cause.post_control_failure(), Some(Code::Cancelled));
    assert_eq!(alias.code(), Code::Read);
    assert_eq!(alias.post_control_failure(), None);
    let original = cause
        .source()
        .and_then(Error::source)
        .and_then(|source| source.downcast_ref::<DiagnosticNativeAllocationFailure>())
        .ok_or("Original inline cause replaced")?;
    assert_eq!(original.operation(), Some(key(2)?));
    assert_eq!(original.code(), Code::Capacity);
    assert_eq!(original.post_control_failure(), None);
    let admitted = DiagnosticRecoveryValidationError::incident_allocation_rejected(cause);
    assert_eq!(admitted.code(), DiagnosticRecoveryValidationCode::InvalidIdentity);
    let stored = admitted
        .source()
        .and_then(|source| source.downcast_ref::<DiagnosticStorageError>())
        .ok_or("Admission source replaced")?;
    assert_eq!(stored.operation(), Some(key(1)?));
    assert_eq!(stored.code(), Code::Read);
    assert_eq!(stored.post_control_failure(), Some(Code::Cancelled));
    let original = stored
        .source()
        .and_then(Error::source)
        .and_then(|source| source.downcast_ref::<DiagnosticNativeAllocationFailure>())
        .ok_or("Admission original source replaced")?;
    assert_eq!(original.operation(), Some(key(2)?));
    assert_eq!(original.code(), Code::Capacity);
    assert!(size_of::<DiagnosticRecoveryValidationError>() <= 120);
    assert!(size_of::<DiagnosticStorageError>() <= 120);
    Ok(())
}

#[test]
fn storage_error_wrapper_alias_keeps_funded_original_cause_and_claim() -> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = setup(&context, Filesystem::new())?;
        let grant = operation.shared._allocation.native_grant();
        let destination = NativeCauseDestination::diagnostic(grant.native_cause()?, grant.clone());
        let cause = DiagnosticStorageError::native(
            Some(operation.key()),
            Code::ChangedIdentity,
            ClaimError::native(
                "storage_metadata",
                BoundedNativeCause::RawIo(std::io::Error::from(
                    std::io::ErrorKind::PermissionDenied,
                )),
                destination,
            ),
        );
        let alias = cause.clone();
        let cause =
            cause.with_post_control(Some(DiagnosticStorageError::rejected(None, Code::Deadline)));
        assert_eq!(cause.post_control_failure(), Some(Code::Deadline));
        assert_eq!(alias.post_control_failure(), None);
        assert_eq!(alias.code(), Code::ChangedIdentity);
        assert_eq!(
            cause
                .source()
                .and_then(Error::source)
                .and_then(|source| source.downcast_ref::<std::io::Error>())
                .ok_or("Native source replaced")?
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        drop(grant);
        assert!(context.memory().claim(1).is_err());
        drop(cause);
        assert!(context.memory().claim(1).is_err());
        drop(alias);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn validation_categories_and_public_ordinary_causes_remain_exact() -> TestResult {
    use DiagnosticRecoveryValidationCode as Validation;
    for code in [
        Validation::InvalidSchema,
        Validation::InvalidJournal,
        Validation::LedgerMismatch,
        Validation::ArithmeticOverflow,
        Validation::InvalidIdentity,
    ] {
        let rejected = DiagnosticRecoveryValidationError::rejected(code);
        assert_eq!(rejected.code(), code);
        assert!(rejected.source().is_none());
        let ordinary = DiagnosticRecoveryValidationError::caused(
            code,
            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        );
        let alias = ordinary.clone();
        drop(ordinary);
        assert_eq!(alias.code(), code);
        assert_eq!(
            alias
                .source()
                .and_then(|source| source.downcast_ref::<std::io::Error>())
                .ok_or("Ordinary concrete cause replaced")?
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }
    Ok(())
}
