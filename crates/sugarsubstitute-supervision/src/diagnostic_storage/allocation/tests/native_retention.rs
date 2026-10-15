// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual cumulative native role loans and final original claim retention.
use super::*;
use std::error::Error;

#[test]
fn scan_generations_saturate_and_reuse_only_after_actual_retention_drop() -> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = setup(&context, Filesystem::new())?;
        let original = operation.shared._allocation.native_grant();
        let mut held: [Option<DiagnosticNativeScanRetention>; 32] = std::array::from_fn(|_| None);
        let mut count = 0;
        let mut rejected = false;
        for slot in &mut held {
            match original.scan_retention(&context) {
                Ok(retention) => {
                    *slot = Some(retention);
                    count += 1;
                }
                Err(cause) => {
                    assert_eq!(cause.code(), Code::Capacity);
                    rejected = true;
                    break;
                }
            }
        }
        assert!(rejected, "Native scan generations escaped residual bound");
        assert!(count >= 2, "Cannot admit actual predecessor and successor");
        let first = held[0].take().ok_or("Missing first generation")?;
        let alias = first.clone();
        drop(first);
        assert_eq!(
            original.scan_retention(&context).err().ok_or("Live alias released capacity")?.code(),
            Code::Capacity
        );
        drop(alias);
        held[0] = Some(original.scan_retention(&context)?);
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        drop(original);
        assert!(context.memory().claim(1).is_err());
        drop(held);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn native_scan_last_alias_retains_claim_after_storage_and_job_binding_drop() -> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = setup(&context, Filesystem::new())?;
        let original = operation.shared._allocation.native_grant();
        let generation = original.scan_retention(&context)?;
        let alias = generation.clone();
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        drop(original);
        drop(generation);
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
fn actual_port_box_and_outside_loan_drop_before_original_claim_returns() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let owner = allocation::owner(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?)?;
        let original = owner.native_grant();
        let loan = original.streaming_owner()?;
        let port = OwnedPort::new(Box::new(filesystem.clone()), Some(loan));
        assert_eq!(std::sync::Arc::strong_count(&filesystem.0), 2);
        drop(owner);
        drop(original);
        assert!(context.memory().claim(1).is_err());
        drop(port);
        assert_eq!(std::sync::Arc::strong_count(&filesystem.0), 1);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn rejected_constructor_destroys_empty_port_box_before_outside_credit_returns() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let owner = allocation::owner(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?)?;
        let original = owner.native_grant();
        let loan = original.streaming_owner()?;
        let port = OwnedPort::new(Box::new(filesystem.clone()), Some(loan));
        owner.close();
        let cause =
            DiagnosticStorage::from_port(owner, port).err().ok_or("Closed constructor admitted")?;
        assert_eq!(cause.code(), Code::Admission);
        assert_eq!(std::sync::Arc::strong_count(&filesystem.0), 1);
        assert_eq!(
            cause
                .source()
                .and_then(std::error::Error::source)
                .and_then(|source| source.downcast_ref::<DiagnosticNativeAllocationFailure>())
                .ok_or("Closed inline cause replaced")?
                .code(),
            Code::Closed
        );
        drop(original);
        drop(cause);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn final_actual_storage_alias_keeps_port_box_and_outside_original_credit() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let owner = allocation::owner(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?)?;
        let original = owner.native_grant();
        let loan = original.streaming_owner()?;
        let port = OwnedPort::new(Box::new(filesystem.clone()), Some(loan));
        let mut storage = DiagnosticStorage::from_port(owner, port)?;
        let operation = storage.begin_operation(key(1)?, &context)?;
        let alias = operation.shared.clone();
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        drop(original);
        assert_eq!(std::sync::Arc::strong_count(&filesystem.0), 2);
        assert!(context.memory().claim(1).is_err());
        drop(alias);
        assert_eq!(std::sync::Arc::strong_count(&filesystem.0), 1);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn native_stage_generation_saturates_and_keeps_original_until_its_unique_last_drop() -> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = setup(&context, Filesystem::new())?;
        let original = operation.shared._allocation.native_grant();
        let mut held: [Option<DiagnosticNativeOwnerLoan>; 8] = std::array::from_fn(|_| None);
        let mut rejected = false;
        let mut count = 0;
        for slot in &mut held {
            match original.stage_scan_owner(&context) {
                Ok(loan) => {
                    *slot = Some(loan);
                    count += 1;
                }
                Err(cause) => {
                    assert_eq!(cause.code(), Code::Capacity);
                    rejected = true;
                    break;
                }
            }
        }
        assert!(rejected);
        assert!(count >= 2);
        let first = held[0].take().ok_or("Actual stage generation absent")?;
        assert_eq!(
            original
                .stage_scan_owner(&context)
                .err()
                .ok_or("Live stage owner released credit")?
                .code(),
            Code::Capacity
        );
        drop(first);
        held[0] = Some(original.stage_scan_owner(&context)?);
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        drop(original);
        assert!(context.memory().claim(1).is_err());
        drop(held);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}
