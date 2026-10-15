// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual protected original preservation and same-v2 normal release.
use super::*;
use crate::{
    coordinator_entry::fixture::Harness,
    profiles::storage::{ProfilePort, WindowsProfilePort},
    protected_storage::record_file,
};
use std::sync::atomic::{AtomicU64, Ordering};
use sugarsubstitute_contracts::{
    backend_custody::{BackendCustodyKey, BackendCustodySession, ProfileCustodyKey},
    execution::{ExecutionPort, JobOutcome, Moment},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
#[test]
fn legacy_released_original_is_protected_before_v2_activation_and_survives_normal_release()
-> Result<(), Box<dyn std::error::Error>> {
    let mut harness = Harness::new()?;
    let until = Moment::from_nanos(15_000_000_000);
    let mut spec = harness.spec(until)?;
    spec.reservation_bytes = crate::profiles::PROFILE_OPERATION_BYTES;
    let directory = std::env::temp_dir().join(format!(
        "sugarsubstitute-profile-migration-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut job = harness.client.submit_blocking(&harness.scope, spec, move |context| {
        let directory = crate::protected_storage::profile_fixture::create(&directory, &context)?;
        let path = directory.join("profile");
        let run = RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])
                    .map_err(|cause| ClaimError::new("migration_epoch", cause))?,
            },
            instance: InstanceId::new(1)
                .map_err(|cause| ClaimError::new("migration_instance", cause))?,
            generation: RunGeneration::new(1)
                .map_err(|cause| ClaimError::new("migration_generation", cause))?,
        };
        let port = WindowsProfilePort;
        let mut previous = NativeState::new();
        port.acquire(&mut previous, &path, run, &context)?;
        port.release(&mut previous, &context)?;
        let marker = path.join(".sugarsubstitute-profile-v1.record");
        let original: [u8; 160] = std::fs::read(&marker)
            .map_err(|cause| ClaimError::new("migration_original", cause))?
            .try_into()
            .map_err(|_| ClaimError::invalid("migration_original_size"))?;
        drop(previous);
        let mut state = NativeState::new();
        port.acquire(&mut state, &path, run, &context)?;
        assert_eq!(state.original_released, Some(original));
        preserve(&mut state, &context)?;
        preserve(&mut state, &context)?;
        let backup = path.join(".sugarsubstitute-profile-v1.original.record");
        assert_eq!(
            std::fs::read(&backup).map_err(|cause| ClaimError::new("migration_backup", cause))?,
            original
        );
        let held = state.record.ok_or_else(|| ClaimError::invalid("migration_held"))?;
        let prospective = record::custody::CustodiedRecord::new(
            held,
            BackendCustodyKey::new(
                run.owner,
                BackendCustodySession::new(1)
                    .map_err(|cause| ClaimError::new("migration_session", cause))?,
            ),
            ProfileCustodyKey::new(run, 9)
                .map_err(|cause| ClaimError::new("migration_family", cause))?,
        )?;
        let file = state.file.as_ref().ok_or_else(|| ClaimError::invalid("migration_writer"))?;
        record_file::publish(
            file,
            &prospective.encode(),
            record_file::PublicationAuthority::Ordinary,
            &context,
            |old| match old {
                record_file::RecordRead::Present(bytes) if *bytes == held.encode() => Ok(()),
                _ => Err(ClaimError::invalid("migration_changed_test_source")),
            },
        )?;
        state.custody_record = Some(prospective);
        port.release(&mut state, &context)?;
        assert_eq!(
            std::fs::read(&marker).map_err(|cause| ClaimError::new("migration_released", cause))?,
            prospective.released().encode()
        );
        assert_eq!(
            std::fs::read(&backup)
                .map_err(|cause| ClaimError::new("migration_preserved", cause))?,
            original
        );
        drop(state);
        for name in [
            ".sugarsubstitute-profile-v1.record",
            ".sugarsubstitute-profile-v1.claim",
            ".sugarsubstitute-profile-v1.original.record",
            ".sugarsubstitute-profile-v1.original.claim",
        ] {
            std::fs::remove_file(path.join(name))
                .map_err(|cause| ClaimError::new("migration_owned_file_cleanup", cause))?;
        }
        std::fs::remove_dir(path)
            .map_err(|cause| ClaimError::new("migration_owned_profile_cleanup", cause))?;
        std::fs::remove_dir(directory)
            .map_err(|cause| ClaimError::new("migration_owned_directory_cleanup", cause))?;
        Ok::<_, ClaimError>(())
    })?;
    let done = harness.consume_job(&mut job)?;
    assert!(matches!(done.outcome, JobOutcome::Success(())), "{done:?}");
    drop((done, job));
    harness.close(until)?;
    harness.join_execution_after_retirement()?;
    assert_eq!(harness.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
