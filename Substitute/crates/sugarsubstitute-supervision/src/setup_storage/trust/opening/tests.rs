// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise protected offline authority, continuity and verification floors through real jobs.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult, installation};
use sugarsubstitute_update::fixture::*;

#[test]
fn installed_authority_retains_floors_without_distribution_or_checkpoint_rewrite() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(31, context)?;
        let signer = key(31)?;
        let root = root(&signer, 1, None)?;
        let store = TrustStore::initialize(installation.path(), [31; 32], &root, context)?;
        store.refresh(Set::new(&signer, [3; 3], b"current", EXPIRES)?.metadata(), NOW, context)?;
        drop(store);
        let checkpoint = installation.path().join("state").join(DIRECTORY).join(RECORD);
        let before = std::fs::read(&checkpoint)?;
        let reopened = TrustStore::open_installed(installation.path(), context)?;
        assert_eq!(reopened.repository, [31; 32]);
        assert_eq!(std::fs::read(&checkpoint)?, before);
        assert!(matches!(
            reopened.refresh(Set::new(&signer, [2; 3], b"old", EXPIRES)?.metadata(), NOW, context),
            Err(TrustFailure::Verification(sugarsubstitute_update::Error::Rollback))
        ));
        drop(reopened);
        assert!(TrustStore::open(installation.path(), [32; 32], context).is_err());
        Ok(())
    })
}

#[test]
fn installed_authority_requires_existing_continuity_without_recreating_it() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(32, context)?;
        drop(TrustStore::initialize(
            installation.path(),
            [32; 32],
            &root(&key(32)?, 1, None)?,
            context,
        )?);
        let marker = installation.path().join("state").join(continuity::MARKER);
        std::fs::remove_file(&marker)?;
        let checkpoint = installation.path().join("state").join(DIRECTORY).join(RECORD);
        let before = std::fs::read(&checkpoint)?;
        assert!(TrustStore::open_installed(installation.path(), context).is_err());
        assert!(!marker.exists());
        assert_eq!(std::fs::read(checkpoint)?, before);
        Ok(())
    })
}

#[test]
fn installed_authority_rejects_validly_encoded_foreign_installation_binding() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(33, context)?;
        let store = TrustStore::initialize(
            installation.path(),
            [33; 32],
            &root(&key(33)?, 1, None)?,
            context,
        )?;
        let path = installation.path().join("state").join(DIRECTORY).join(RECORD);
        let original = std::fs::read(&path)?;
        let (_, payload) =
            envelope::decode_binding(envelope::Kind::Checkpoint, &store.installation, &original)?;
        let foreign = envelope::encode(
            envelope::Kind::Checkpoint,
            &InstallationId::new(format!("{:032x}", 34))?,
            &[33; 32],
            payload,
        )?;
        drop(store);
        std::fs::write(&path, &foreign)?;
        assert!(TrustStore::open_installed(installation.path(), context).is_err());
        assert_eq!(std::fs::read(path)?, foreign);
        Ok(())
    })
}

#[test]
fn installed_authority_rejects_checkpoint_damage_and_missing_state_without_reset() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let installation = installation(35, context)?;
        assert!(TrustStore::open_installed(installation.path(), context).is_err());
        assert!(!installation.path().join("state").join(DIRECTORY).exists());
        drop(TrustStore::initialize(
            installation.path(),
            [35; 32],
            &root(&key(35)?, 1, None)?,
            context,
        )?);
        let path = installation.path().join("state").join(DIRECTORY).join(RECORD);
        let mut bytes = std::fs::read(&path)?;
        bytes[41] ^= 1;
        std::fs::write(&path, &bytes)?;
        assert!(TrustStore::open_installed(installation.path(), context).is_err());
        assert_eq!(std::fs::read(path)?, bytes);
        Ok(())
    })
}

#[test]
fn inferred_repository_must_agree_with_history_and_original_continuity() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        for replace_history in [false, true] {
            let installation = installation(40, context)?;
            let store = TrustStore::initialize(
                installation.path(),
                [40; 32],
                &root(&key(40)?, 1, None)?,
                context,
            )?;
            let identity = store.installation.clone();
            drop(store);
            let checkpoint = installation.path().join("state").join(DIRECTORY).join(RECORD);
            let history = installation.path().join("state").join(DIRECTORY).join(releases::RECORD);
            let marker = installation.path().join("state").join(continuity::MARKER);
            let marker_before = std::fs::read(&marker)?;
            for (path, kind) in
                [(&checkpoint, envelope::Kind::Checkpoint), (&history, envelope::Kind::Releases)]
            {
                if path == &history && !replace_history {
                    continue;
                }
                let original = std::fs::read(path)?;
                let (_, payload) = envelope::decode_binding(kind, &identity, &original)?;
                std::fs::write(path, envelope::encode(kind, &identity, &[41; 32], payload)?)?;
            }
            let checkpoint_before = std::fs::read(&checkpoint)?;
            let history_before = std::fs::read(&history)?;
            assert!(TrustStore::open_installed(installation.path(), context).is_err());
            assert_eq!(std::fs::read(checkpoint)?, checkpoint_before);
            assert_eq!(std::fs::read(history)?, history_before);
            assert_eq!(std::fs::read(marker)?, marker_before);
        }
        Ok(())
    })
}
