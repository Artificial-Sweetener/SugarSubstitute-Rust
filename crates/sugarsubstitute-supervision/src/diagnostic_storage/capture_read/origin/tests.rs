// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pure owner predicate tests do not fabricate native root pins or checked read proofs.
use super::*;
use std::error::Error;
use sugarsubstitute_contracts::{
    execution::ExecutionPort, incident::IncidentOperationId, lifecycle::*,
    recovery_supervisor::RecoveryGrantId,
};
fn record() -> Result<RecordedRecoveryCaptureOrigin, Box<dyn Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let run = RunKey { owner, instance: InstanceId::new(3)?, generation: RunGeneration::new(4)? };
    Ok(RecordedRecoveryCaptureOrigin::new(
        LogicalStoreOperationKey::new(owner, IncidentOperationId::new(5)?),
        RecoveryCaptureIdentity::new(run, RecoveryGrantId::new(6)?),
        [7; 24],
        BuildIdentity::new("historical-build")?,
    )?)
}
#[test]
fn checked_origin_root_predicate_accepts_new_root_epoch_only_with_exact_original_native_identity()
-> Result<(), Box<dyn Error>> {
    let record = record()?;
    let successor =
        CoordinatorKey { epoch: CoordinatorEpoch::new([9; 16])?, ..record.operation.owner() };
    assert_ne!(successor, record.operation.owner());
    assert!(matches_current_root(&record, successor, Some([7; 24])));
    assert!(!matches_current_root(&record, successor, Some([8; 24])));
    assert!(!matches_current_root(&record, successor, None));
    let foreign = CoordinatorKey { namespace: NamespaceId::new([8; 32]), ..successor };
    assert!(!matches_current_root(&record, foreign, Some([7; 24])));
    Ok(())
}
#[test]
fn recorded_origin_is_only_data_and_rejects_foreign_original_root_or_absent_root()
-> Result<(), Box<dyn Error>> {
    let record = record()?;
    let foreign = LogicalStoreOperationKey::new(
        CoordinatorKey { epoch: CoordinatorEpoch::new([9; 16])?, ..record.operation.owner() },
        record.operation.id(),
    );
    assert!(
        RecordedRecoveryCaptureOrigin::new(
            foreign,
            record.origin,
            record.root_identity,
            record.build.clone()
        )
        .is_err()
    );
    assert!(
        RecordedRecoveryCaptureOrigin::new(
            record.operation,
            record.origin,
            [0; 24],
            record.build.clone()
        )
        .is_err()
    );
    Ok(())
}

struct DecoderBoundary(RecordedRecoveryCaptureOrigin);
impl RecoveryCaptureOriginValidator for DecoderBoundary {
    fn validate(
        &self,
        _: &mut DiagnosticRecoveryDecodeLoan,
        _: &DiagnosticRecoverySource,
    ) -> Result<RecordedRecoveryCaptureOrigin, DiagnosticRecoveryValidationError> {
        Ok(self.0.clone())
    }
}
#[test]
fn actual_origin_reader_cannot_issue_checked_opening_from_decoder_facts_without_real_root_pins()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::execution::{JobOutcome, Moment};
    let mut rig = crate::coordinator_entry::fixture::Harness::new()?;
    let mut spec = rig.spec(Moment::from_nanos(10_000_000_000))?;
    spec.reservation_bytes = 1024 * 1024;
    let mut job = rig.client.submit_blocking(&rig.scope, spec, |context| {
        crate::ProtectedStorageRoot::new(
            context
                .memory()
                .claim(1024 * 1024)
                .map_err(|cause| ClaimError::new("source_actual_claim", cause))?,
        )
    })?;
    let completed = rig.consume_job(&mut job)?;
    let mut root = match completed.outcome {
        JobOutcome::Success(root) => root,
        _ => return Err("root owner admission failed".into()),
    };
    drop(completed.memory);
    drop(job);
    let record = record()?;
    let mut spec = rig.spec(Moment::from_nanos(10_000_000_000))?;
    spec.reservation_bytes = DIAGNOSTIC_STORAGE_BYTES;
    let mut job = rig.client.submit_blocking(&rig.scope, spec, move |context| {
        let fixture = crate::diagnostic_storage::fixture::DiagnosticStorageFixture::new()?;
        fixture
            .put(DiagnosticObject::Journal(record.operation), b"external decoder fixture bytes")?;
        let mut storage = fixture.storage(&context)?;
        let operation = storage.begin_operation(record.operation, &context)?;
        fixture.resolve_opening(&mut storage, &operation, &context)?;
        storage.acquire_writer(&operation, &context)?;
        storage.inventory(&operation, &context)?;
        let read = storage.read(
            DiagnosticObject::Journal(record.operation),
            4096,
            &operation,
            &context,
        )?;
        let before = fixture.snapshot().mutation_count();
        assert!(
            storage
                .check_recovery_capture_origin(
                    &root,
                    read,
                    &DecoderBoundary(record.clone()),
                    &operation,
                    &context
                )
                .is_err()
        );
        assert_eq!(fixture.snapshot().mutation_count(), before);
        assert_eq!(storage.snapshot().reads(), 1);
        assert!(root.root_identity().is_none());
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        root.retire(&context)
            .map_err(|cause| DiagnosticStorageError::native(None, Code::Retirement, cause))?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok::<_, DiagnosticStorageError>(())
    })?;
    let completed = rig.consume_job(&mut job)?;
    assert!(
        matches!(completed.outcome, JobOutcome::Success(())),
        "origin owner result: {:?}",
        completed.outcome
    );
    drop(completed);
    drop(job);
    rig.close(Moment::from_nanos(60_000_000_000))?;
    rig.join_execution_after_retirement()?;
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn origin_preservation_classification_uses_actual_current_scan_and_full_retained_file_facts()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::execution::{JobOutcome, Moment};
    let mut rig = crate::coordinator_entry::fixture::Harness::new()?;
    let record = record()?;
    let mut spec = rig.spec(Moment::from_nanos(10_000_000_000))?;
    spec.reservation_bytes = DIAGNOSTIC_STORAGE_BYTES;
    let mut job = rig.client.submit_blocking(&rig.scope, spec, move |context| {
        let fixture = crate::diagnostic_storage::fixture::DiagnosticStorageFixture::new()?;
        let object = DiagnosticObject::Journal(record.operation);
        fixture.put(object, b"external strict decoder fixture")?;
        let mut storage = fixture.storage(&context)?;
        let operation = storage.begin_operation(record.operation, &context)?;
        fixture.resolve_opening(&mut storage, &operation, &context)?;
        storage.acquire_writer(&operation, &context)?;
        storage.inventory(&operation, &context)?;
        let mut read = storage.read(object, 4096, &operation, &context)?;
        storage.recognize_recovery_capture_origin(
            &mut read,
            &DecoderBoundary(record.clone()),
            &operation,
            &context,
        )?;
        {
            let data = lock(&storage.shared);
            let entry = data
                .scan_entries
                .iter()
                .find(|entry| entry.class == DiagnosticEntryClass::Journal)
                .ok_or_else(|| error(operation.key, Code::InvalidIdentity))?;
            assert!(recognized_origin(&data, entry));
            let mut changed = entry.clone();
            changed.identity[0] ^= 1;
            assert!(!recognized_origin(&data, &changed));
            changed = entry.clone();
            changed.digest[0] ^= 1;
            assert!(!recognized_origin(&data, &changed));
            changed = entry.clone();
            changed.bytes += 1;
            assert!(!recognized_origin(&data, &changed));
        }
        read.retire(&context)?;
        storage.inventory(&operation, &context)?;
        {
            let data = lock(&storage.shared);
            assert!(!data.scan_entries.iter().any(|entry| recognized_origin(&data, entry)));
        }
        let mut read = storage.read(object, 4096, &operation, &context)?;
        storage.recognize_recovery_capture_origin(
            &mut read,
            &DecoderBoundary(record.clone()),
            &operation,
            &context,
        )?;
        read.retire(&context)?;
        storage.retire(&operation, &context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok::<_, DiagnosticStorageError>(())
    })?;
    let completed = rig.consume_job(&mut job)?;
    assert!(
        matches!(completed.outcome, JobOutcome::Success(())),
        "preservation classification: {:?}",
        completed.outcome
    );
    drop(completed);
    drop(job);
    rig.close(Moment::from_nanos(60_000_000_000))?;
    rig.join_execution_after_retirement()?;
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

struct PromotionDecoderBoundary(RecordedRecoveryCaptureOrigin);
impl CapturePromotionValidator for PromotionDecoderBoundary {
    fn validate(
        &self,
        _: &mut DiagnosticRecoveryDecodeLoan,
        _: &DiagnosticRecoverySource,
    ) -> Result<RecordedCapturePromotion, DiagnosticRecoveryValidationError> {
        RecordedCapturePromotion::new(
            self.0.operation(),
            sugarsubstitute_contracts::crash::CaptureGenerationOrigin::Coordinator(
                self.0.operation().owner(),
            ),
            [[1; 24], [2; 24]],
        )
    }
}
#[test]
fn actual_promotion_reader_rejects_decoder_facts_without_genuine_current_root_pins()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::execution::{JobOutcome, Moment};
    let mut rig = crate::coordinator_entry::fixture::Harness::new()?;
    let mut spec = rig.spec(Moment::from_nanos(10_000_000_000))?;
    spec.reservation_bytes = 1024 * 1024;
    let mut job = rig.client.submit_blocking(&rig.scope, spec, |context| {
        crate::ProtectedStorageRoot::new(
            context
                .memory()
                .claim(1024 * 1024)
                .map_err(|cause| ClaimError::new("promotion_original_claim", cause))?,
        )
    })?;
    let completed = rig.consume_job(&mut job)?;
    let mut root = match completed.outcome {
        JobOutcome::Success(root) => root,
        _ => return Err("root owner rejected".into()),
    };
    drop(completed.memory);
    drop(job);
    let record = record()?;
    let mut spec = rig.spec(Moment::from_nanos(10_000_000_000))?;
    spec.reservation_bytes = DIAGNOSTIC_STORAGE_BYTES;
    let mut job = rig.client.submit_blocking(&rig.scope, spec, move |context| {
        let fixture = crate::diagnostic_storage::fixture::DiagnosticStorageFixture::new()?;
        let object = DiagnosticObject::Journal(record.operation());
        fixture.put(object, b"external complete promotion decoder fixture")?;
        let current = LogicalStoreOperationKey::new(
            CoordinatorKey {
                epoch: CoordinatorEpoch::new([9; 16]).map_err(|cause| {
                    native(
                        record.operation(),
                        Code::InvalidIdentity,
                        ClaimError::new("promotion_epoch", cause),
                    )
                })?,
                ..record.operation().owner()
            },
            record.operation().id(),
        );
        let mut storage = fixture.storage(&context)?;
        let operation = storage.begin_operation(current, &context)?;
        fixture.resolve_opening(&mut storage, &operation, &context)?;
        storage.acquire_writer(&operation, &context)?;
        storage.inventory(&operation, &context)?;
        let mut read = storage.read(object, 4096, &operation, &context)?;
        let mut output = None;
        let before = fixture.snapshot().mutation_count();
        assert!(
            storage
                .prepare_capture_promotion_read(
                    &mut root,
                    &mut read,
                    &PromotionDecoderBoundary(record.clone()),
                    &mut output,
                    &operation,
                    &context
                )
                .is_err()
        );
        assert!(output.is_none());
        assert!(root.root_identity().is_none());
        assert_eq!(fixture.snapshot().mutation_count(), before);
        assert_eq!(storage.snapshot().reads(), 1);
        read.retire(&context)?;
        storage.retire(&operation, &context)?;
        root.retire(&context)
            .map_err(|cause| DiagnosticStorageError::native(None, Code::Retirement, cause))?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok::<_, DiagnosticStorageError>(())
    })?;
    let completed = rig.consume_job(&mut job)?;
    assert!(
        matches!(completed.outcome, JobOutcome::Success(())),
        "promotion rejection: {:?}",
        completed.outcome
    );
    drop(completed);
    drop(job);
    rig.close(Moment::from_nanos(60_000_000_000))?;
    rig.join_execution_after_retirement()?;
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
