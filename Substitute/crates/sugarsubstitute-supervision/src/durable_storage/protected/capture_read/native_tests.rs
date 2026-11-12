// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify real protected generation duplication through two genuine admitted jobs.
use super::*;
use crate::diagnostic_storage::{DIAGNOSTIC_STORAGE_BYTES, fixture::ProtectedCaptureSourceFixture};
use std::sync::atomic::{AtomicU64, Ordering};
use sugarsubstitute_contracts::{
    incident::{IncidentOperationId, LogicalStoreOperationKey},
    lifecycle::{CoordinatorEpoch, NamespaceId},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
#[test]
fn capture_source_fixture_rejects_existing_path_and_oversized_storage_without_adoption()
-> TestResult {
    let mut rig = crate::coordinator_entry::fixture::Harness::new()?;
    let coordinator = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([1; 16])?,
    };
    let mut spec = rig.spec(Moment::from_nanos(10_000_000_000))?;
    spec.reservation_bytes = ProtectedCaptureSourceFixture::required_reservation_bytes();
    let mut job = rig.client.submit_blocking(&rig.scope, spec, move |context| {
        let mut oversized = PathBuf::new();
        oversized.try_reserve_exact(65_537)?;
        let rejected = ProtectedCaptureSourceFixture::create(&context, coordinator, oversized)
            .err()
            .ok_or("Oversized fixture storage admitted")?;
        assert_eq!(rejected.operation(), "fixture_path_capacity");
        let path = std::env::temp_dir().join(format!(
            "sugarsubstitute-existing-source-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        let rejected = ProtectedCaptureSourceFixture::create(&context, coordinator, path.clone())
            .err()
            .ok_or("Existing fixture namespace adopted")?;
        assert_eq!(rejected.operation(), "fixture_create_new");
        assert!(!path.join("protected").exists());
        drop(rejected);
        assert_eq!(context.memory().claim(STORAGE_BYTES)?.bytes(), STORAGE_BYTES);
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    })?;
    let done = rig.consume_job(&mut job)?;
    assert!(
        matches!(done.outcome, JobOutcome::Success(())),
        "Actual fixture rejection result: {:?}",
        done.outcome
    );
    drop(done);
    drop(job);
    rig.close(Moment::from_nanos(60_000_000_000))?;
    rig.join_execution_after_retirement()?;
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn native_capture_read_uses_actual_generation_pins_and_separate_original_operation() -> TestResult {
    let mut rig = crate::coordinator_entry::fixture::Harness::new()?;
    let coordinator = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([1; 16])?,
    };
    let mut source_spec = rig.spec(Moment::from_nanos(10_000_000_000))?;
    source_spec.reservation_bytes = STORAGE_BYTES;
    let mut source_job = rig.client.submit_blocking(&rig.scope, source_spec, move |context| {
        ProtectedCaptureSourceFixture::create(
            &context,
            coordinator,
            std::env::temp_dir().join(format!(
                "sugarsubstitute-capture-read-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )),
        )
    })?;
    let completion = rig.consume_job(&mut source_job)?;
    let mut source = match completion.outcome {
        JobOutcome::Success(source) => source,
        JobOutcome::Failure(cause) => return Err(cause.into()),
        _ => return Err("Actual protected source construction stopped".into()),
    };
    drop(completion.memory);
    drop(source_job);
    let mut current_spec = rig.spec(Moment::from_nanos(10_000_000_000))?;
    current_spec.reservation_bytes = DIAGNOSTIC_STORAGE_BYTES;
    let mut current_job = rig.client.submit_blocking(&rig.scope, current_spec, move |context| {
        let fixture = crate::diagnostic_storage::fixture::DiagnosticStorageFixture::new()?;
        let mut storage = fixture.storage(&context)?;
        let operation = storage.begin_operation(
            LogicalStoreOperationKey::new(
                coordinator,
                IncidentOperationId::new(1)
                    .map_err(|cause| ClaimError::new("fixture_operation", cause))?,
            ),
            &context,
        )?;
        storage.resolve(
            Path::new("external-storage-fixture"),
            coordinator.namespace,
            &operation,
            &context,
        )?;
        storage.acquire_writer(&operation, &context)?;
        drop(storage.inventory(&operation, &context)?);
        let mut output = None;
        storage.prepare_capture_read(source.root_mut(), &mut output, &operation, &context)?;
        assert_eq!(storage.snapshot().reads(), 1);
        let lease = output.as_mut().ok_or_else(|| ClaimError::invalid("fixture_read_lease"))?;
        let before = storage.snapshot();
        let (finished, rust_retention) =
            storage.reserve_finished_owner(lease, &operation, &context)?;
        assert_eq!(finished.operation(), operation.key());
        assert_eq!(rust_retention.operation(), operation.key());
        assert!(rust_retention.matches_cpp(&finished));
        assert_eq!(storage.snapshot().live_handles(), before.live_handles() + 11);
        assert_eq!(storage.snapshot().stages(), before.stages() + 2);
        assert_eq!(
            storage
                .reserve_finished_owner(lease, &operation, &context)
                .err()
                .ok_or("Second Finished owner admitted")?
                .code(),
            crate::diagnostic_storage::DiagnosticStorageFailureCode::Capacity,
        );
        assert_eq!(
            lease.retire(&context).err().ok_or("Finished owner lost source custody")?.code(),
            crate::diagnostic_storage::DiagnosticStorageFailureCode::Busy,
        );
        assert_eq!(storage.snapshot().reads(), 1);
        let journal = crate::diagnostic_storage::DiagnosticObject::Journal(operation.key());
        for _ in 0..6 {
            drop(storage.stage(journal, 64, &operation, &context)?);
        }
        assert_eq!(storage.snapshot().stages(), 8);
        let mutations = fixture.snapshot().mutation_count();
        assert_eq!(
            storage
                .stage(journal, 64, &operation, &context)
                .err()
                .ok_or("Finished metadata reservation exceeded eight stages")?
                .code(),
            crate::diagnostic_storage::DiagnosticStorageFailureCode::Capacity,
        );
        assert_eq!(fixture.snapshot().mutation_count(), mutations);
        let held = storage.snapshot();
        drop(finished);
        assert_eq!(storage.snapshot().live_handles(), held.live_handles() - 11);
        assert_eq!(storage.snapshot().stages(), held.stages() - 2);
        let identities = {
            let guard = lease.borrow(&context)?;
            assert_eq!(guard.coordinator()?, coordinator);
            let identities = guard.identities()?;
            assert_ne!(identities[0], [0; 24]);
            assert_ne!(identities[0], identities[1]);
            guard.database()?;
            guard.metrics()?;
            guard.check_current()?;
            identities
        };
        std::fs::write(
            source.directory().join("qualification.txt"),
            format!("database={:?}\nmetrics={:?}\n", identities[0], identities[1]),
        )
        .map_err(|cause| ClaimError::new("fixture_evidence", cause))?;
        lease.retire(&context)?;
        drop(output);
        assert_eq!(storage.snapshot().reads(), 0);
        source.retire(&context)?;
        assert!(source.is_retired());
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        drop(source);
        assert!(context.memory().claim(1).is_err());
        drop(rust_retention);
        assert_eq!(
            context
                .memory()
                .claim(DIAGNOSTIC_STORAGE_BYTES)
                .map_err(|cause| ClaimError::new("fixture_returned_claim", cause))?
                .bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    })?;
    let done = rig.consume_job(&mut current_job)?;
    assert!(
        matches!(done.outcome, JobOutcome::Success(())),
        "Actual read owner result: {:?}",
        done.outcome
    );
    drop(done);
    drop(current_job);
    rig.close(Moment::from_nanos(60_000_000_000))?;
    rig.join_execution_after_retirement()?;
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
