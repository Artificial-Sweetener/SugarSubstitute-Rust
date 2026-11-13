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
    let mut pending = WriteReceipt::new(operation, 104);
    assert_eq!(
        WriteReceipt::written(Some(&pending), operation, 104),
        Err(ObservationError::WriteOrder)
    );
    WriteReceipt::issued(Some(&mut pending), operation)?;
    WriteReceipt::written(Some(&pending), operation, 104)?;
    Ok(())
}

#[test]
fn foreign_write_issuance_cannot_consume_original_issuance() -> TestResult {
    let operation = LifecyclePipeOperationId::new(4)?;
    let mut pending = WriteReceipt::new(operation, 104);
    assert_eq!(
        WriteReceipt::issued(Some(&mut pending), LifecyclePipeOperationId::new(5)?),
        Err(ObservationError::WriteIdentity)
    );
    WriteReceipt::issued(Some(&mut pending), operation)?;
    WriteReceipt::written(Some(&pending), operation, 104)?;
    Ok(())
}

#[test]
fn duplicate_write_issuance_preserves_original_completion() -> TestResult {
    let operation = LifecyclePipeOperationId::new(4)?;
    let mut pending = WriteReceipt::new(operation, 104);
    WriteReceipt::issued(Some(&mut pending), operation)?;
    assert_eq!(
        WriteReceipt::issued(Some(&mut pending), operation),
        Err(ObservationError::WriteOrder)
    );
    WriteReceipt::written(Some(&pending), operation, 104)?;
    Ok(())
}

#[test]
fn foreign_completion_and_inexact_sizes_preserve_original_completion() -> TestResult {
    let operation = LifecyclePipeOperationId::new(4)?;
    let mut pending = WriteReceipt::new(operation, 104);
    WriteReceipt::issued(Some(&mut pending), operation)?;
    assert_eq!(
        WriteReceipt::written(Some(&pending), LifecyclePipeOperationId::new(5)?, 104),
        Err(ObservationError::WriteIdentity)
    );
    for bytes in [0, 103, 105, u32::MAX] {
        assert_eq!(
            WriteReceipt::written(Some(&pending), operation, bytes),
            Err(ObservationError::WriteBytes)
        );
    }
    WriteReceipt::written(Some(&pending), operation, 104)?;
    Ok(())
}

#[test]
fn write_observations_after_record_release_are_rejected() -> TestResult {
    let operation = LifecyclePipeOperationId::new(4)?;
    assert_eq!(WriteReceipt::issued(None, operation), Err(ObservationError::WriteIdentity));
    assert_eq!(WriteReceipt::written(None, operation, 104), Err(ObservationError::WriteIdentity));
    Ok(())
}

#[test]
fn acquisition_native_write_retirement_requires_unissued_exact_operation()
-> Result<(), Box<dyn std::error::Error>> {
    let original = LifecyclePipeOperationId::new(1)?;
    let foreign = LifecyclePipeOperationId::new(2)?;
    let mut receipt = WriteReceipt::new(original, 73);
    assert_eq!(receipt.bytes(), 73);
    assert_eq!(receipt.revoked(foreign), Err(ObservationError::WriteIdentity));
    receipt.revoked(original)?;
    WriteReceipt::issued(Some(&mut receipt), original)?;
    assert_eq!(receipt.revoked(original), Err(ObservationError::WriteOrder));
    assert_eq!(
        WriteReceipt::written(Some(&receipt), original, 72),
        Err(ObservationError::WriteBytes)
    );
    WriteReceipt::written(Some(&receipt), original, 73)?;
    Ok(())
}

#[test]
fn acquisition_uncertain_or_issued_native_veto_never_grants_local_retirement() {
    use sugarsubstitute_contracts::execution::Moment;
    use sugarsubstitute_execution::windows_lifecycle_pipe::NativeWriteRevocation::*;
    assert!(WriteReceipt::permits_no_issue(RevokedBeforeIssue));
    assert!(WriteReceipt::permits_no_issue(AlreadyRevoked));
    for result in [
        Retired,
        AlreadyIssued { at: Moment::from_nanos(1) },
        IssueUncertain { at: Moment::from_nanos(1) },
    ] {
        assert!(!WriteReceipt::permits_no_issue(result));
    }
}
