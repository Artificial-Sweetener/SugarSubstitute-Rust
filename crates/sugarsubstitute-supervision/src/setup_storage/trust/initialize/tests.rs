// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Interrupted bootstrap never exposes partial trust or replaces a foreign native directory.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult, installation};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_update::fixture::{key, root};

#[test]
fn missing_initialized_namespace_cannot_reset_accepted_signing_history() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(47, context)?;
        let initial_root = root(&key(39)?, 1, None)?;
        let store = TrustStore::initialize(installation.path(), [39; 32], &initial_root, context)?;
        store.rotate(&root(&key(48)?, 2, Some(&key(39)?))?, context)?;
        let original = std::fs::read(installation.path().join(DIRECTORY).join(RECORD))?;
        drop(store);
        let displaced = installation.path().join("preserved-trust");
        std::fs::rename(installation.path().join(DIRECTORY), &displaced)?;
        assert!(
            TrustStore::initialize(installation.path(), [39; 32], &initial_root, context).is_err()
        );
        assert!(!installation.path().join(DIRECTORY).exists());
        assert_eq!(std::fs::read(displaced.join(RECORD))?, original);
        Ok(())
    })
}

#[test]
fn public_initialization_exposes_only_complete_trust_at_operation_boundaries() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 8 * 1024 * 1024, |context, clock| {
        let installation = installation(45, context)?;
        let trusted_root = root(&key(39)?, 1, None)?;
        let (checkpoint, history) = records(installation.path(), &trusted_root, context)?;
        let path = installation.path().to_owned();
        let staged = Arc::new(AtomicBool::new(false));
        let published = Arc::new(AtomicBool::new(false));
        let incomplete = Arc::new(AtomicBool::new(false));
        let observations = (staged.clone(), published.clone(), incomplete.clone());
        clock.observe_checks(move || {
            if let Ok(entries) = std::fs::read_dir(&path) {
                for entry in entries.flatten() {
                    if entry.file_name().to_string_lossy().starts_with(".update-trust-stage-") {
                        observations.0.store(true, Ordering::SeqCst);
                    }
                }
            }
            let destination = path.join(DIRECTORY);
            if destination.exists() {
                observations.1.store(true, Ordering::SeqCst);
                // The empty rendezvous can be byte-range locked during continuity publication.
                if !std::fs::symlink_metadata(destination.join(LOCK))
                    .is_ok_and(|metadata| metadata.is_file() && metadata.len() == 0)
                {
                    observations.2.store(true, Ordering::SeqCst);
                }
                for (name, expected) in
                    [(RECORD, checkpoint.as_slice()), (releases::RECORD, history.as_slice())]
                {
                    if !std::fs::read(destination.join(name)).is_ok_and(|bytes| bytes == expected) {
                        observations.2.store(true, Ordering::SeqCst);
                    }
                }
            }
            false
        })?;
        let store = TrustStore::initialize(installation.path(), [39; 32], &trusted_root, context)?;
        assert_eq!(store.next_root_version(context)?, 2);
        assert!(staged.load(Ordering::SeqCst));
        assert!(published.load(Ordering::SeqCst));
        assert!(!incomplete.load(Ordering::SeqCst));
        Ok(())
    })
}

#[test]
fn public_initialization_interrupted_during_staging_leaves_no_authoritative_trust() -> TestResult {
    worker::run_reserved_expiring(TRUST_RESERVATION_BYTES, |context, clock| {
        let installation = installation(46, context)?;
        let trusted_root = root(&key(39)?, 1, None)?;
        let path = installation.path().to_owned();
        let interrupted = Arc::new(AtomicBool::new(false));
        let observed = interrupted.clone();
        clock.observe_checks(move || {
            let found = std::fs::read_dir(&path).is_ok_and(|entries| {
                entries.flatten().any(|entry| {
                    entry.file_name().to_string_lossy().starts_with(".update-trust-stage-")
                })
            });
            if found {
                observed.store(true, Ordering::SeqCst);
            }
            found
        })?;
        assert!(
            TrustStore::initialize(installation.path(), [39; 32], &trusted_root, context).is_err()
        );
        assert!(interrupted.load(Ordering::SeqCst));
        assert!(!installation.path().join(DIRECTORY).exists());
        Ok(())
    })
}

fn records(
    installation: &Path,
    trusted_root: &[u8],
    context: &JobContext,
) -> Result<(Vec<u8>, Vec<u8>), Box<dyn std::error::Error + Send + Sync>> {
    let selected =
        SelectionStore::open(installation, context)?.load(context)?.ok_or("Selection absent")?;
    let checkpoint = RepositoryTrust::bootstrap(trusted_root)?.checkpoint()?;
    Ok((
        envelope::encode(
            envelope::Kind::Checkpoint,
            selected.installation(),
            &[39; 32],
            &checkpoint,
        )?,
        envelope::encode(
            envelope::Kind::Releases,
            selected.installation(),
            &[39; 32],
            &sugarsubstitute_update::ReleaseHistory::empty().encode()?,
        )?,
    ))
}

#[test]
fn abandoned_initialization_at_each_file_boundary_cannot_lock_out_fresh_trust() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 8 * 1024 * 1024, |context, _| {
        let trusted_root = root(&key(39)?, 1, None)?;
        for count in 0..=3 {
            let installation = installation(39, context)?;
            let parent = Directory::open(installation.path(), false)?;
            let (checkpoint, history) = records(&parent.path, &trusted_root, context)?;
            let orphan = parent.path.join(".update-trust-stage-interrupted");
            let records = [
                (LOCK, b"".as_slice()),
                (releases::RECORD, history.as_slice()),
                (RECORD, checkpoint.as_slice()),
            ];
            protection::with(context, |protection| {
                protection.directory(&orphan, Mode::CreateNew, context)?;
                for (name, bytes) in records.iter().take(count) {
                    let mut file = protection.file(&orphan.join(name), context)?;
                    file.write_all(bytes)?;
                    file.sync_all()?;
                }
                Ok(())
            })?;
            let store = TrustStore::initialize(&parent.path, [39; 32], &trusted_root, context)?;
            assert_eq!(store.next_root_version(context)?, 2);
            assert_eq!(std::fs::read_dir(&orphan)?.count(), count);
            for (name, bytes) in records.iter().take(count) {
                assert_eq!(std::fs::read(orphan.join(name))?, *bytes);
            }
            assert_eq!(std::fs::read(parent.path.join(DIRECTORY).join(RECORD))?, checkpoint);
        }
        Ok(())
    })
}

#[test]
fn cancellation_before_directory_publication_preserves_unpublished_state() -> TestResult {
    worker::run_reserved_expiring(TRUST_RESERVATION_BYTES, |context, clock| {
        let installation = installation(40, context)?;
        let parent = Directory::open(installation.path(), false)?;
        let staged = InitialState::stage(&parent, b"checkpoint", b"history", context)?;
        let saved = staged.path.clone();
        clock.expire();
        assert!(staged.publish(&parent.path.join(DIRECTORY), context).is_err());
        assert!(!parent.path.join(DIRECTORY).exists());
        assert_eq!(std::fs::read(saved.join(RECORD))?, b"checkpoint");
        assert_eq!(std::fs::read(saved.join(releases::RECORD))?, b"history");
        Ok(())
    })
}

#[test]
fn replacing_a_staging_path_with_identical_bytes_cannot_publish_another_directory() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(41, context)?;
        let parent = Directory::open(installation.path(), false)?;
        let staged = InitialState::stage(&parent, b"checkpoint", b"history", context)?;
        let saved = parent.path.join("original-stage");
        std::fs::rename(&staged.path, &saved)?;
        protection::with(context, |protection| {
            protection.directory(&staged.path, Mode::CreateNew, context)?;
            for (name, bytes) in [
                (RECORD, b"checkpoint".as_slice()),
                (releases::RECORD, b"history".as_slice()),
                (LOCK, b"".as_slice()),
            ] {
                let mut file = protection.file(&staged.path.join(name), context)?;
                file.write_all(bytes)?;
                file.sync_all()?;
            }
            Ok(())
        })?;
        let replacement = staged.path.clone();
        assert!(staged.publish(&parent.path.join(DIRECTORY), context).is_err());
        assert!(!parent.path.join(DIRECTORY).exists());
        assert_eq!(std::fs::read(saved.join(RECORD))?, b"checkpoint");
        assert_eq!(std::fs::read(replacement.join(RECORD))?, b"checkpoint");
        Ok(())
    })
}

#[test]
fn publication_never_replaces_an_existing_directory_or_its_files() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(42, context)?;
        let parent = Directory::open(installation.path(), false)?;
        let staged = InitialState::stage(&parent, b"checkpoint", b"history", context)?;
        let source = staged.path.clone();
        let destination = parent.path.join(DIRECTORY);
        std::fs::create_dir(&destination)?;
        std::fs::write(destination.join("user-file"), b"preserve")?;
        assert!(staged.publish(&destination, context).is_err());
        assert_eq!(std::fs::read(source.join(RECORD))?, b"checkpoint");
        assert_eq!(std::fs::read(destination.join("user-file"))?, b"preserve");
        assert!(!destination.join(RECORD).exists());
        Ok(())
    })
}

#[test]
fn publication_rejects_unprotected_staging_and_releases_its_native_handle() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(44, context)?;
        let path = installation.path().join("unprotected-stage");
        std::fs::create_dir(&path)?;
        std::fs::write(path.join("user-file"), b"preserve")?;
        let identity = Directory::open(&path, false)?.identity()?;
        let staged = InitialState { path: path.clone(), identity };
        let destination = installation.path().join(DIRECTORY);
        assert!(staged.publish(&destination, context).is_err());
        assert!(!destination.exists());
        let moved = installation.path().join("still-user-owned");
        std::fs::rename(&path, &moved)?;
        assert_eq!(std::fs::read(moved.join("user-file"))?, b"preserve");
        Ok(())
    })
}

#[test]
fn crash_after_publication_leaves_complete_trust_that_reopens_without_initializing_again()
-> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(43, context)?;
        let parent = Directory::open(installation.path(), false)?;
        let trusted_root = root(&key(39)?, 1, None)?;
        let (checkpoint, history) = records(&parent.path, &trusted_root, context)?;
        let staged = InitialState::stage(&parent, &checkpoint, &history, context)?;
        let source = staged.path.clone();
        drop(staged.publish(&parent.path.join(DIRECTORY), context)?);
        assert!(!source.exists());
        let store = TrustStore::open(&parent.path, [39; 32], context)?;
        assert_eq!(store.next_root_version(context)?, 2);
        assert!(TrustStore::initialize(&parent.path, [39; 32], &trusted_root, context).is_err());
        assert_eq!(std::fs::read(parent.path.join(DIRECTORY).join(RECORD))?, checkpoint);
        assert_eq!(std::fs::read(parent.path.join(DIRECTORY).join(releases::RECORD))?, history);
        Ok(())
    })
}
