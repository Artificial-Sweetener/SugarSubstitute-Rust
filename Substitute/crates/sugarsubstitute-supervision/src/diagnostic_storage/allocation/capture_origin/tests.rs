// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The real original diagnostic claim survives failed close and escaped origin metadata.
use super::*;
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::{
    execution::{ExecutionPort, JobOutcome, Moment},
    incident::{IncidentOperationId, LogicalStoreOperationKey},
    lifecycle::*,
};
#[test]
fn origin_metadata_loan_retains_actual_original_capacity_through_failed_native_boundary_retirement()
-> Result<(), Box<dyn std::error::Error>> {
    let mut rig = crate::coordinator_entry::fixture::Harness::new()?;
    let mut spec = rig.spec(Moment::from_nanos(10_000_000_000))?;
    spec.reservation_bytes = DIAGNOSTIC_STORAGE_BYTES;
    let mut job = rig.client.submit_blocking(&rig.scope, spec, move |context| {
        let fixture = crate::diagnostic_storage::fixture::DiagnosticStorageFixture::new()?;
        let mut storage = fixture.storage(&context)?;
        let owner = CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16]).map_err(|cause| {
                DiagnosticStorageError::native(
                    None,
                    Code::Admission,
                    ClaimError::new("test_identity", cause),
                )
            })?,
        };
        let id = IncidentOperationId::new(1).map_err(|cause| {
            DiagnosticStorageError::native(
                None,
                Code::Admission,
                ClaimError::new("test_identity", cause),
            )
        })?;
        let operation =
            storage.begin_operation(LogicalStoreOperationKey::new(owner, id), &context)?;
        let mut opening = fixture.resolve_opening(&mut storage, &operation, &context)?;
        storage.acquire_writer(&operation, &context)?;
        storage.inventory(&operation, &context)?;
        let DiagnosticStoreOpening::Created(genesis) = &mut opening else {
            return Err(DiagnosticStorageError::rejected(None, Code::InvalidIdentity));
        };
        let stage = storage.stage_genesis(
            genesis,
            Sha256::digest(b"ledger").into(),
            6,
            &operation,
            &context,
        )?;
        assert_eq!(fixture.snapshot().live_outputs(), 1);
        let loan = storage.recovery_capture_origin(&operation, &context)?;
        fixture
            .set_failure(crate::diagnostic_storage::fixture::DiagnosticFixtureFailure::Close, true);
        assert!(storage.retire(&operation, &context).is_err());
        assert!(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES).is_err());
        assert!(fixture.snapshot().live_outputs() > 0);
        fixture.set_failure(
            crate::diagnostic_storage::fixture::DiagnosticFixtureFailure::Close,
            false,
        );
        storage.retire(&operation, &context)?;
        drop(stage);
        drop(opening);
        drop(operation);
        drop(storage);
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        assert!(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES).is_err());
        drop(loan);
        let full = context.memory().claim(DIAGNOSTIC_STORAGE_BYTES).map_err(|cause| {
            DiagnosticStorageError::native(
                None,
                Code::Admission,
                ClaimError::new("test_actual_credit", cause),
            )
        })?;
        assert_eq!(full.bytes(), DIAGNOSTIC_STORAGE_BYTES);
        drop(full);
        Ok::<_, DiagnosticStorageError>(())
    })?;
    let completed = rig.consume_job(&mut job)?;
    assert!(
        matches!(completed.outcome, JobOutcome::Success(())),
        "origin metadata owner result: {:?}",
        completed.outcome
    );
    drop(completed);
    drop(job);
    rig.close(Moment::from_nanos(60_000_000_000))?;
    rig.join_execution_after_retirement()?;
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
