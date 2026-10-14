// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual genesis custody through real execution and external filesystem faults.
mod retained_stage;
use super::*;
use sha2::{Digest, Sha256};
fn fresh(
    context: &JobContext,
    filesystem: Filesystem,
) -> Result<(DiagnosticStorage, DiagnosticStorageOperation, DiagnosticStoreOpening), TestError> {
    let allocation = allocation::owner(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?)?;
    let filesystem = filesystem.retain_claim(&allocation)?;
    let mut storage =
        DiagnosticStorage::from_port(allocation, OwnedPort::new(Box::new(filesystem), None))?;
    let operation = storage.begin_operation(key(1)?, context)?;
    let mut state = storage.check(&operation, context)?;
    let opening = state.call_native(|port| {
        port.resolve(Path::new("external-fake"), operation.key.owner().namespace, context)
    })??;
    let opening = storage.adopt_opening(state, opening, &operation, context)?;
    storage.acquire_writer(&operation, context)?;
    storage.inventory(&operation, context)?;
    Ok((storage, operation, opening))
}
fn created(opening: DiagnosticStoreOpening) -> Result<DiagnosticStoreGenesis, TestError> {
    match opening {
        DiagnosticStoreOpening::Created(value) => Ok(value),
        _ => Err("fresh directory not observed".into()),
    }
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
#[test]
fn genesis_actual_created_directory_issues_one_matching_ledger_stage() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation, opening) = fresh(&context, filesystem.clone())?;
        let mut proof = created(opening)?;
        let mut stage =
            storage.stage_genesis(&mut proof, digest(b"ledger"), 6, &operation, &context)?;
        stage.write_chunk(b"ledger", &context)?;
        let flushed = stage.flush(&context)?.read_facts();
        let activation =
            storage.activate(&mut stage, DiagnosticPredecessor::Absent, &operation, &context)?;
        assert_eq!(activation.facts(), flushed);
        storage.inventory(&operation, &context)?;
        let before =
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations;
        assert!(
            storage.stage_genesis(&mut proof, digest(b"ledger"), 6, &operation, &context).is_err()
        );
        assert_eq!(
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
            before
        );
        drop(activation);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn genesis_existing_missing_or_unknown_store_never_gains_absent_activation() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        // Existing directory with no ledger is an actual external observation.
        filesystem.clone().resolve(
            Path::new("external-existing"),
            key(1)?.owner().namespace,
            &context,
        )?;
        let (mut storage, operation, opening) = fresh(&context, filesystem.clone())?;
        assert!(matches!(opening, DiagnosticStoreOpening::Existing));
        let mut stage = storage.stage(DiagnosticObject::StoreState, 6, &operation, &context)?;
        stage.write_chunk(b"ledger", &context)?;
        stage.flush(&context)?;
        let before =
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations;
        assert_eq!(
            storage
                .activate(&mut stage, DiagnosticPredecessor::Absent, &operation, &context)
                .err()
                .ok_or("ordinary stage initialized existing store")?
                .code(),
            Code::InvalidIdentity
        );
        assert_eq!(
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
            before
        );
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn genesis_hash_size_and_nonempty_inventory_preflight_precede_native_markers() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation, opening) = fresh(&context, filesystem.clone())?;
        let mut proof = created(opening)?;
        for (hash, bytes) in [([0; 32], 6), (digest(b"ledger"), 0), (digest(b"ledger"), 65537)] {
            assert!(storage.stage_genesis(&mut proof, hash, bytes, &operation, &context).is_err());
        }
        assert_eq!(
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
            0
        );
        filesystem.put(DiagnosticObject::Summary(incident()?), b"unknown-before-genesis");
        storage.inventory(&operation, &context)?;
        assert!(
            storage.stage_genesis(&mut proof, digest(b"ledger"), 6, &operation, &context).is_err()
        );
        assert_eq!(
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
            0
        );
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn genesis_mismatched_bytes_keep_original_intent_without_second_stage() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation, opening) = fresh(&context, filesystem.clone())?;
        let mut proof = created(opening)?;
        let mut stage =
            storage.stage_genesis(&mut proof, digest(b"ledger"), 6, &operation, &context)?;
        stage.write_chunk(b"mutate", &context)?;
        stage.flush(&context)?;
        let before =
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations;
        assert!(
            storage
                .activate(&mut stage, DiagnosticPredecessor::Absent, &operation, &context)
                .is_err()
        );
        assert!(
            storage.stage_genesis(&mut proof, digest(b"mutate"), 6, &operation, &context).is_err()
        );
        assert_eq!(
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
            before
        );
        assert_eq!(storage.retained_genesis_stage(&mut proof, &operation, &context)?.id, stage.id);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn genesis_partial_create_retains_original_claim_and_blocks_repeat_create() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation, opening) = fresh(&context, filesystem.clone())?;
        let mut proof = created(opening)?;
        filesystem
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .faults
            .partial_stage = true;
        assert!(
            storage.stage_genesis(&mut proof, digest(b"ledger"), 6, &operation, &context).is_err()
        );
        assert_eq!(storage.snapshot().stages(), 1);
        assert!(context.memory().claim(1).is_err());
        let before =
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations;
        assert!(
            storage.stage_genesis(&mut proof, digest(b"ledger"), 6, &operation, &context).is_err()
        );
        assert!(storage.retained_genesis_stage(&mut proof, &operation, &context).is_err());
        assert_eq!(
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
            before
        );
        storage.retire(&operation, &context)?;
        drop(proof);
        drop(operation);
        drop(storage);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}
#[test]
fn genesis_post_rename_uncertainty_reconciles_same_intent_after_checked_close() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation, opening) = fresh(&context, filesystem.clone())?;
        let mut proof = created(opening)?;
        let mut stage =
            storage.stage_genesis(&mut proof, digest(b"ledger"), 6, &operation, &context)?;
        stage.write_chunk(b"ledger", &context)?;
        let facts = stage.flush(&context)?.read_facts();
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.activate =
            true;
        assert_eq!(
            storage
                .activate(&mut stage, DiagnosticPredecessor::Absent, &operation, &context)
                .err()
                .ok_or("uncertain rename succeeded")?
                .code(),
            Code::ActivationUncertain
        );
        assert_eq!(storage.snapshot().stages(), 1);
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close = true;
        let expected = DiagnosticPredecessor::Exact {
            file_identity: facts.file_identity(),
            sha256: facts.sha256(),
        };
        assert!(
            storage
                .reconcile_activation(DiagnosticObject::StoreState, expected, &operation, &context)
                .is_err()
        );
        assert_eq!(storage.snapshot().stages(), 1);
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close = false;
        let activation = storage.reconcile_activation(
            DiagnosticObject::StoreState,
            expected,
            &operation,
            &context,
        )?;
        assert_eq!(activation.facts(), facts);
        assert_eq!(storage.snapshot().stages(), 0);
        storage.inventory(&operation, &context)?;
        assert!(
            storage.stage_genesis(&mut proof, digest(b"ledger"), 6, &operation, &context).is_err()
        );
        drop(activation);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}

#[test]
fn genesis_public_namespace_request_uses_actual_entry_owner_and_retains_closed_source_fence()
-> Result<(), Box<dyn std::error::Error>> {
    let mut entry = crate::coordinator_entry::fixture::Harness::new()?;
    entry.discover()?;
    let namespace = entry.owner.take_namespace().ok_or("actual namespace not produced")?;
    let current = LogicalStoreOperationKey::new(
        CoordinatorKey { namespace: namespace.namespace(), epoch: CoordinatorEpoch::new([2; 16])? },
        IncidentOperationId::new(1)?,
    );
    let foreign = LogicalStoreOperationKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([2; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        IncidentOperationId::new(1)?,
    );
    assert!(namespace.diagnostic_write_request(foreign).is_err());
    let request = namespace.diagnostic_write_request(current)?;
    let lifetime = std::sync::Arc::clone(&request.lifetime);
    let mut spec = entry.spec(Moment::from_nanos(10_000_000_000))?;
    spec.reservation_bytes = DIAGNOSTIC_STORAGE_BYTES;
    let mut job = entry.client.submit_blocking(&entry.scope, spec, move |context| {
        let fixture = super::super::fixture::DiagnosticStorageFixture::new()?;
        let mut storage = fixture.storage(&context)?;
        let operation = storage.begin_operation(current, &context)?;
        let mut proof = match storage.resolve_namespace(&request, &operation, &context)? {
            DiagnosticStoreOpening::Created(proof) => proof,
            _ => return Err(error(current, Code::InvalidIdentity)),
        };
        storage.acquire_writer(&operation, &context)?;
        storage.inventory(&operation, &context)?;
        let before = fixture.snapshot().mutation_count();
        lifetime.close_admission();
        assert_eq!(
            storage
                .stage_genesis(&mut proof, digest(b"ledger"), 6, &operation, &context)
                .err()
                .ok_or_else(|| error(current, Code::InvalidIdentity))?
                .code(),
            Code::Closed
        );
        assert_eq!(fixture.snapshot().mutation_count(), before);
        storage.retire(&operation, &context)?;
        drop(proof);
        drop(operation);
        drop(storage);
        assert!(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES).is_ok());
        Ok::<_, DiagnosticStorageError>(())
    })?;
    let done = entry.consume_job(&mut job)?;
    assert!(
        matches!(done.outcome, JobOutcome::Success(())),
        "actual storage job: {:?}",
        done.outcome
    );
    drop(done);
    drop(job);
    drop(namespace);
    entry.close(Moment::from_nanos(60_000_000_000))?;
    entry.join_execution_after_retirement()?;
    assert_eq!(entry.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn genesis_public_fixture_rejects_foreign_disk_before_resolution() -> TestResult {
    admitted(|context, _| {
        let fixture = super::super::fixture::DiagnosticStorageFixture::new()?;
        let foreign = super::super::fixture::DiagnosticStorageFixture::new()?;
        let mut storage = fixture.storage(&context)?;
        let operation = storage.begin_operation(key(1)?, &context)?;
        assert!(foreign.resolve_opening(&mut storage, &operation, &context).is_err());
        assert_eq!(fixture.snapshot().mutation_count(), 0);
        assert_eq!(foreign.snapshot().mutation_count(), 0);
        let mut genesis = created(fixture.resolve_opening(&mut storage, &operation, &context)?)?;
        storage.acquire_writer(&operation, &context)?;
        storage.inventory(&operation, &context)?;
        let mut stage =
            storage.stage_genesis(&mut genesis, digest(b"ledger"), 6, &operation, &context)?;
        stage.write_chunk(b"ledger", &context)?;
        stage.flush(&context)?;
        storage.activate(&mut stage, DiagnosticPredecessor::Absent, &operation, &context)?;
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
