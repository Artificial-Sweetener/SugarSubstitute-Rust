// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Child diagnostic completeness never substitutes for process/family settlement.
use super::*;
fn complete() -> ChildReceipt {
    ChildReceipt {
        schema_version: 1,
        role: "coordinator-contender".into(),
        correlation: 1,
        original_deadline_qpc: 200,
        qpc_frequency: 100,
        original_execution_deadline_nanoseconds: 100,
        namespace: None,
        epoch: None,
        actual_failure_operation: Some("claim_exclusion".into()),
        backend_acquired: false,
        backend_released: false,
        claims_closed: true,
        inheritance_cleared: true,
        ready_signalled: true,
        response_signalled: true,
        release_observed: true,
        async_finished: true,
        cpu_finished: true,
        native_finished: true,
        reserved_bytes_after_teardown: 0,
        native_teardown_finished: true,
    }
}
#[test]
fn exact_child_receipt_rejects_wrong_identity_operation_and_partial_teardown() -> Result<()> {
    let receipt = complete();
    receipt.validate(Role::CoordinatorContender, 1, 200, 100)?;
    assert!(receipt.validate(Role::CoordinatorContender, 2, 200, 100).is_err());
    assert!(receipt.validate(Role::CoordinatorContender, 1, 201, 100).is_err());
    let mut broken = receipt.clone();
    broken.actual_failure_operation = Some("unresolved_claim_predecessor".into());
    assert!(broken.validate(Role::CoordinatorContender, 1, 200, 100).is_err());
    let mut broken = receipt.clone();
    broken.claims_closed = false;
    assert!(broken.validate(Role::CoordinatorContender, 1, 200, 100).is_err());
    let mut broken = receipt;
    broken.reserved_bytes_after_teardown = 1;
    assert!(broken.validate(Role::CoordinatorContender, 1, 200, 100).is_err());
    Ok(())
}
#[test]
fn receipt_schema_rejects_unknown_fields_and_denial_success_confusion() -> Result<()> {
    let mut value = serde_json::to_value(complete())?;
    value["untrusted_extra"] = true.into();
    assert!(serde_json::from_value::<ChildReceipt>(value).is_err());
    let mut receipt = complete();
    receipt.role = "backend-success".into();
    receipt.actual_failure_operation = None;
    assert!(receipt.validate(Role::BackendSuccess, 1, 200, 100).is_err());
    Ok(())
}
#[test]
fn backend_success_requires_acquisition_release_and_canonical_fresh_owner() -> Result<()> {
    let mut receipt = complete();
    receipt.role = Role::BackendSuccess.wire().into();
    receipt.actual_failure_operation = None;
    receipt.namespace = Some("ab".repeat(32));
    receipt.epoch = Some("cd".repeat(16));
    receipt.backend_acquired = true;
    receipt.backend_released = true;
    receipt.validate(Role::BackendSuccess, 1, 200, 100)?;
    let mut invalid = receipt.clone();
    invalid.backend_released = false;
    assert!(invalid.validate(Role::BackendSuccess, 1, 200, 100).is_err());
    let mut invalid = receipt.clone();
    invalid.epoch = Some("00".repeat(16));
    assert!(invalid.validate(Role::BackendSuccess, 1, 200, 100).is_err());
    let mut invalid = receipt;
    invalid.namespace = Some("AB".repeat(32));
    assert!(invalid.validate(Role::BackendSuccess, 1, 200, 100).is_err());
    Ok(())
}
