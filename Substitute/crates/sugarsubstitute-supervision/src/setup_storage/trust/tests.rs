// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual protected Windows files and signature verification through scoped jobs.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult, installation};
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::installation::{
    Generation, GenerationId, PackageDigest, PreparationIntent,
};
use sugarsubstitute_update::{Error as Verification, fixture::*};
mod publication;

/// The fixed UTC Unix time records are prepared at here; no test reads it back.
const PREPARED_AT: u64 = 1_800_000_000;

#[test]
fn restart_preserves_signature_floors_and_cannot_reinitialize_trust() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(1, context)?;
        let key = key(1)?;
        let root = root(&key, 1, None)?;
        let repository = Sha256::digest(&root).into();
        let current = Set::new(&key, [2; 3], b"current", EXPIRES)?;
        {
            let store = TrustStore::initialize(
                installation.path(),
                Sha256::digest(&root).into(),
                &root,
                context,
            )?;
            store.refresh(current.metadata(), NOW, context)?;
        }
        assert!(
            TrustStore::initialize(
                installation.path(),
                Sha256::digest(&root).into(),
                &root,
                context
            )
            .is_err()
        );
        let store = TrustStore::open(installation.path(), repository, context)?;
        assert!(matches!(
            store.refresh(Set::new(&key, [1; 3], b"old", EXPIRES)?.metadata(), NOW, context),
            Err(TrustFailure::Verification(Verification::Rollback))
        ));
        store.refresh(current.metadata(), NOW, context)?;
        Ok(())
    })
}

#[test]
fn failed_child_verification_persists_the_authenticated_parent_floor() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(2, context)?;
        let key = key(2)?;
        let root = root(&key, 1, None)?;
        let repository = Sha256::digest(&root).into();
        {
            let store = TrustStore::initialize(
                installation.path(),
                Sha256::digest(&root).into(),
                &root,
                context,
            )?;
            let current = Set::new(&key, [8; 3], b"current", EXPIRES)?;
            assert!(matches!(
                store.refresh(
                    MetadataSet {
                        timestamp: &current.timestamp,
                        snapshot: b"broken",
                        targets: &current.targets,
                    },
                    NOW,
                    context
                ),
                Err(TrustFailure::Verification(_))
            ));
        }
        let store = TrustStore::open(installation.path(), repository, context)?;
        assert!(matches!(
            store.refresh(Set::new(&key, [7; 3], b"old", EXPIRES)?.metadata(), NOW, context,),
            Err(TrustFailure::Verification(Verification::Rollback))
        ));
        store.refresh(Set::new(&key, [8; 3], b"current", EXPIRES)?.metadata(), NOW, context)?;
        Ok(())
    })
}

#[test]
fn root_rotation_survives_reopen_and_revokes_old_signers() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(3, context)?;
        let old = key(3)?;
        let next = key(4)?;
        let root = root(&old, 1, None)?;
        let repository = Sha256::digest(&root).into();
        {
            let store = TrustStore::initialize(
                installation.path(),
                Sha256::digest(&root).into(),
                &root,
                context,
            )?;
            store.rotate(&sugarsubstitute_update::fixture::root(&next, 2, Some(&old))?, context)?;
        }
        let store = TrustStore::open(installation.path(), repository, context)?;
        assert!(
            store
                .refresh(Set::new(&old, [1; 3], b"revoked", EXPIRES)?.metadata(), NOW, context)
                .is_err()
        );
        store.refresh(Set::new(&next, [1; 3], b"current", EXPIRES)?.metadata(), NOW, context)?;
        Ok(())
    })
}

#[test]
fn writer_exclusion_releases_without_stale_file_lockout_or_application_lease() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(4, context)?;
        let key = key(5)?;
        let root = root(&key, 1, None)?;
        let store = TrustStore::initialize(
            installation.path(),
            Sha256::digest(&root).into(),
            &root,
            context,
        )?;
        let current = Set::new(&key, [1; 3], b"current", EXPIRES)?;
        // A trust writer does not acquire installation activation or selection exclusion.
        let setup = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(installation.path().join("state/setup-lock"))?;
        setup.try_lock().map_err(io::Error::from)?;
        assert!(SelectionStore::open(installation.path(), context)?.load(context)?.is_some());
        store.refresh(current.metadata(), NOW, context)?;
        assert!(store.root.path().join(LOCK).exists());
        Ok(())
    })
}

#[test]
fn corrupt_missing_and_foreign_state_is_preserved_without_bootstrap_fallback() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(5, context)?;
        let root = root(&key(6)?, 1, None)?;
        let repository = Sha256::digest(&root).into();
        let store = TrustStore::initialize(
            installation.path(),
            Sha256::digest(&root).into(),
            &root,
            context,
        )?;
        let path = store.root.path().join(RECORD);
        let original = std::fs::read(&path)?;
        assert!(TrustStore::open(installation.path(), [99; 32], context).is_err());
        for bytes in [b"broken".to_vec(), {
            let mut wrong = original.clone();
            wrong[7] = b'2';
            wrong
        }] {
            std::fs::write(&path, &bytes)?;
            assert!(TrustStore::open(installation.path(), repository, context).is_err());
            assert!(
                TrustStore::initialize(
                    installation.path(),
                    Sha256::digest(&root).into(),
                    &root,
                    context
                )
                .is_err()
            );
            assert_eq!(std::fs::read(&path)?, bytes);
        }
        std::fs::remove_file(&path)?;
        assert!(TrustStore::open(installation.path(), repository, context).is_err());
        assert!(
            TrustStore::initialize(
                installation.path(),
                Sha256::digest(&root).into(),
                &root,
                context
            )
            .is_err()
        );
        assert!(!path.exists());
        Ok(())
    })
}

#[test]
fn checkpoint_copied_between_installations_is_rejected_and_preserved() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let a = installation(6, context)?;
        let b = installation(7, context)?;
        let root = root(&key(7)?, 1, None)?;
        let repository = Sha256::digest(&root).into();
        let first = TrustStore::initialize(a.path(), Sha256::digest(&root).into(), &root, context)?;
        let second =
            TrustStore::initialize(b.path(), Sha256::digest(&root).into(), &root, context)?;
        let bytes = std::fs::read(first.root.path().join(RECORD))?;
        std::fs::write(second.root.path().join(RECORD), &bytes)?;
        assert!(TrustStore::open(b.path(), repository, context).is_err());
        assert_eq!(std::fs::read(second.root.path().join(RECORD))?, bytes);
        Ok(())
    })
}

#[test]
fn progressed_or_unknown_installations_cannot_start_new_trust_history() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(8, context)?;
        let store = SelectionStore::open(installation.path(), context)?;
        let initial = store.load(context)?.ok_or("Missing fixture selection")?;
        let next = initial.prepare(
            Generation {
                id: GenerationId::new("1".repeat(32))?,
                package: PackageDigest::new("2".repeat(64))?,
                release: sugarsubstitute_contracts::installation::SignedRelease::new(
                    "1.0.0".to_owned(),
                    1,
                )?,
            },
            PreparationIntent::Automatic,
            PREPARED_AT,
        )?;
        store.compare_exchange(&initial, &next, context)?;
        let root = root(&key(8)?, 1, None)?;
        assert!(
            TrustStore::initialize(
                installation.path(),
                Sha256::digest(&root).into(),
                &root,
                context
            )
            .is_err()
        );
        assert!(!installation.path().join("state").join(DIRECTORY).exists());
        let unknown = tempfile::tempdir()?;
        std::fs::write(unknown.path().join("user-file"), b"preserve")?;
        assert!(
            TrustStore::initialize(unknown.path(), Sha256::digest(&root).into(), &root, context)
                .is_err()
        );
        assert_eq!(std::fs::read(unknown.path().join("user-file"))?, b"preserve");
        Ok(())
    })
}

#[test]
fn new_installations_can_bootstrap_current_keys_without_changing_repository_identity() -> TestResult
{
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let old_installation = installation(9, context)?;
        let new_installation = installation(10, context)?;
        let old = key(9)?;
        let next = key(10)?;
        let old_root = root(&old, 1, None)?;
        let next_root = root(&next, 2, Some(&old))?;
        let repository = Sha256::digest(&old_root).into();
        let old_store =
            TrustStore::initialize(old_installation.path(), repository, &old_root, context)?;
        old_store.rotate(&next_root, context)?;
        let new_store =
            TrustStore::initialize(new_installation.path(), repository, &next_root, context)?;
        let metadata = Set::new(&next, [1; 3], b"current", EXPIRES)?;
        old_store.refresh(metadata.metadata(), NOW, context)?;
        new_store.refresh(metadata.metadata(), NOW, context)?;
        drop(new_store);
        let reopened = TrustStore::open(new_installation.path(), repository, context)?;
        reopened.refresh(metadata.metadata(), NOW, context)?;
        Ok(())
    })
}
