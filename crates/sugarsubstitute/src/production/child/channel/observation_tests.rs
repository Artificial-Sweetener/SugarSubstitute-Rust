// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify original transport matching without synthesizing native capabilities.
use super::*;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn read_pending_and_received_can_match_the_same_original_admission() -> TestResult {
    let operation = LifecyclePipeOperationId::new(3)?;
    let pending = PendingRead::new(operation);
    PendingRead::validate(Some(&pending), operation)?;
    PendingRead::validate(Some(&pending), operation)?;
    Ok(())
}

#[test]
fn foreign_read_observations_leave_original_read_eligible() -> TestResult {
    let operation = LifecyclePipeOperationId::new(3)?;
    let pending = PendingRead::new(operation);
    assert_eq!(
        PendingRead::validate(Some(&pending), LifecyclePipeOperationId::new(4)?),
        Err(ObservationError::ReadIdentity)
    );
    PendingRead::validate(Some(&pending), operation)?;
    Ok(())
}

#[test]
fn read_after_original_record_transfer_is_rejected() -> TestResult {
    assert_eq!(
        PendingRead::validate(None, LifecyclePipeOperationId::new(3)?),
        Err(ObservationError::ReadIdentity)
    );
    Ok(())
}

#[test]
fn write_completion_requires_actual_issuance_and_exact_original_bytes() -> TestResult {
    let operation = LifecyclePipeOperationId::new(4)?;
    let mut pending = PendingWrite::new(operation, 104);
    assert_eq!(
        PendingWrite::written(Some(&pending), operation, 104),
        Err(ObservationError::WriteOrder)
    );
    PendingWrite::issued(Some(&mut pending), operation)?;
    PendingWrite::written(Some(&pending), operation, 104)?;
    Ok(())
}

#[test]
fn foreign_write_issuance_cannot_consume_original_issuance() -> TestResult {
    let operation = LifecyclePipeOperationId::new(4)?;
    let mut pending = PendingWrite::new(operation, 104);
    assert_eq!(
        PendingWrite::issued(Some(&mut pending), LifecyclePipeOperationId::new(5)?),
        Err(ObservationError::WriteIdentity)
    );
    PendingWrite::issued(Some(&mut pending), operation)?;
    PendingWrite::written(Some(&pending), operation, 104)?;
    Ok(())
}

#[test]
fn duplicate_write_issuance_preserves_original_completion() -> TestResult {
    let operation = LifecyclePipeOperationId::new(4)?;
    let mut pending = PendingWrite::new(operation, 104);
    PendingWrite::issued(Some(&mut pending), operation)?;
    assert_eq!(
        PendingWrite::issued(Some(&mut pending), operation),
        Err(ObservationError::WriteOrder)
    );
    PendingWrite::written(Some(&pending), operation, 104)?;
    Ok(())
}

#[test]
fn foreign_completion_and_inexact_sizes_preserve_original_completion() -> TestResult {
    let operation = LifecyclePipeOperationId::new(4)?;
    let mut pending = PendingWrite::new(operation, 104);
    PendingWrite::issued(Some(&mut pending), operation)?;
    assert_eq!(
        PendingWrite::written(Some(&pending), LifecyclePipeOperationId::new(5)?, 104),
        Err(ObservationError::WriteIdentity)
    );
    for bytes in [0, 103, 105, u32::MAX] {
        assert_eq!(
            PendingWrite::written(Some(&pending), operation, bytes),
            Err(ObservationError::WriteBytes)
        );
    }
    PendingWrite::written(Some(&pending), operation, 104)?;
    Ok(())
}

#[test]
fn write_observations_after_record_release_are_rejected() -> TestResult {
    let operation = LifecyclePipeOperationId::new(4)?;
    assert_eq!(PendingWrite::issued(None, operation), Err(ObservationError::WriteIdentity));
    assert_eq!(PendingWrite::written(None, operation, 104), Err(ObservationError::WriteIdentity));
    Ok(())
}
