// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the genuine failed allocation cause under its original admitted destination.
use super::*;

#[test]
fn genuine_reservation_cause_and_last_alias_keep_original_claim() -> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = setup(&context, Filesystem::new())?;
        let owner = operation.shared._allocation.clone();
        let destination = allocation::reservation(&owner, Some(operation.key))?;
        let mut buffer = Vec::<u8>::new();
        let source = buffer.try_reserve_exact(usize::MAX).err().ok_or("Overflow admitted")?;
        drop(buffer);
        let cause = destination.failed(source);
        assert_eq!(cause.code(), Code::Capacity);
        let native = std::error::Error::source(&cause).ok_or("Native cause lost")?;
        assert!(
            std::error::Error::source(native)
                .and_then(|source| source.downcast_ref::<std::collections::TryReserveError>())
                .is_some()
        );
        let alias = cause.clone();
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        drop(owner);
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
fn closed_original_reservation_rejects_without_new_cause_or_data_allocation() -> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = setup(&context, Filesystem::new())?;
        let owner = operation.shared._allocation.clone();
        storage.retire(&operation, &context)?;
        let cause = allocation::reservation(&owner, Some(operation.key))
            .err()
            .ok_or("Closed reservation admitted")?;
        assert_eq!(cause.code(), Code::Admission);
        let native = std::error::Error::source(&cause).ok_or("Inline denial lost")?;
        assert!(
            std::error::Error::source(native)
                .and_then(|source| source.downcast_ref::<DiagnosticNativeAllocationFailure>())
                .is_some()
        );
        drop(cause);
        drop(owner);
        drop(operation);
        drop(storage);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}
