// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise installer admission through real protected state and authenticated repository roots.
use super::*;
use crate::setup_storage::{
    Selection,
    test_worker::{self as worker, TestResult},
};
use sugarsubstitute_update::fixture::*;

fn choices(root: &Path) -> Selection {
    Selection {
        locale: "en".into(),
        install_root: root.to_owned(),
        mode: "attached".into(),
        comfy_root: root.join("external-comfy"),
        python: String::new(),
        models: root.join("models"),
        outputs: root.join("outputs"),
        selection_token: None,
    }
}

#[test]
fn new_sessions_allocate_distinct_identities_without_claiming_application_health() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let first = tempfile::tempdir()?;
        let second = tempfile::tempdir()?;
        let trusted_root = root(&key(70)?, 1, None)?;
        let mut identities = Vec::new();
        for path in [first.path(), second.path()] {
            let session = InstallationSession::create(choices(path), context)?;
            let trust = session.update_trust([70; 32], &trusted_root, context)?;
            let record = SelectionStore::open(path, context)?.load(context)?.ok_or("No state")?;
            assert_eq!(record.revision(), 0);
            assert!(record.accepted().is_none());
            assert!(record.candidate().is_none());
            assert_eq!(&trust.installation, record.installation());
            identities.push(record.installation().clone());
            assert!(!path.join("releases").exists());
            assert!(!path.join("external-comfy").exists());
        }
        assert_ne!(identities[0], identities[1]);
        Ok(())
    })
}

#[test]
fn existing_session_authority_ignores_bootstrap_bytes_and_preserves_rotated_keys() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let old = key(70)?;
        let next = key(71)?;
        let original = root(&old, 1, None)?;
        let session = InstallationSession::create(choices(temporary.path()), context)?;
        let trust = session.update_trust([70; 32], &original, context)?;
        trust.rotate(&root(&next, 2, Some(&old))?, context)?;
        let checkpoint = std::fs::read(trust.root.path.join(RECORD))?;
        let identity = trust.installation.clone();
        drop(trust);
        drop(session);
        let reopened = InstallationSession::create(choices(temporary.path()), context)?;
        assert!(reopened.update_trust([71; 32], &original, context).is_err());
        let trust = reopened.update_trust([70; 32], b"unused bootstrap bytes", context)?;
        assert_eq!(trust.installation, identity);
        assert_eq!(std::fs::read(trust.root.path.join(RECORD))?, checkpoint);
        assert!(
            trust
                .refresh(Set::new(&old, [1; 3], b"old", EXPIRES)?.metadata(), NOW, context)
                .is_err()
        );
        trust.refresh(Set::new(&next, [1; 3], b"new", EXPIRES)?.metadata(), NOW, context)?;
        Ok(())
    })
}

#[test]
fn invalid_bootstrap_root_leaves_selection_and_trust_uninitialized() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let session = InstallationSession::create(choices(temporary.path()), context)?;
        assert!(session.update_trust([70; 32], b"invalid root", context).is_err());
        assert!(!temporary.path().join(".selection-state").exists());
        assert!(!temporary.path().join(".selection-enabled.json").exists());
        assert!(!temporary.path().join(DIRECTORY).exists());
        assert!(!temporary.path().join(continuity::MARKER).exists());
        session.update_trust([70; 32], &root(&key(70)?, 1, None)?, context)?;
        Ok(())
    })
}

#[test]
fn insufficient_metadata_admission_cannot_enable_selection() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let session = InstallationSession::create(choices(temporary.path()), context)?;
        assert!(session.update_trust([70; 32], &root(&key(70)?, 1, None)?, context).is_err());
        assert!(!temporary.path().join(".selection-state").exists());
        assert!(!temporary.path().join(DIRECTORY).exists());
        Ok(())
    })
}

#[test]
fn missing_authoritative_records_never_trigger_session_bootstrap() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        let trusted_root = root(&key(70)?, 1, None)?;
        for missing in [
            ".update-trust/checkpoint",
            ".update-trust/releases",
            ".update-trust",
            ".selection-state/.substitute-selection.json",
            ".selection-state",
        ] {
            let temporary = tempfile::tempdir()?;
            let session = InstallationSession::create(choices(temporary.path()), context)?;
            drop(session.update_trust([70; 32], &trusted_root, context)?);
            let marker = std::fs::read(temporary.path().join(continuity::MARKER))?;
            let path = temporary.path().join(missing);
            let saved = temporary.path().join("preserved-state");
            std::fs::rename(&path, &saved)?;
            assert!(session.update_trust([70; 32], &trusted_root, context).is_err(), "{missing}");
            assert!(!path.exists(), "{missing}");
            assert!(saved.exists());
            assert_eq!(std::fs::read(temporary.path().join(continuity::MARKER))?, marker);
        }
        Ok(())
    })
}

#[test]
fn trust_evidence_without_selection_never_allocates_a_replacement_identity() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES, |context, _| {
        for name in [DIRECTORY, continuity::MARKER] {
            let temporary = tempfile::tempdir()?;
            let session = InstallationSession::create(choices(temporary.path()), context)?;
            std::fs::write(temporary.path().join(name), b"unknown state")?;
            assert!(session.update_trust([70; 32], &root(&key(70)?, 1, None)?, context).is_err());
            assert!(!temporary.path().join(".selection-state").exists());
            assert!(!temporary.path().join(".selection-enabled.json").exists());
            assert_eq!(std::fs::read(temporary.path().join(name))?, b"unknown state");
        }
        Ok(())
    })
}

#[test]
fn interruption_after_selection_resumes_with_the_original_installation_identity() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().to_owned();
    worker::run_reserved_expiring(TRUST_RESERVATION_BYTES, move |context, clock| {
        let session = InstallationSession::create(choices(&path), context)?;
        let observed = path.clone();
        clock.observe_checks(move || observed.join(".selection-enabled.json").exists())?;
        assert!(session.update_trust([70; 32], &root(&key(70)?, 1, None)?, context).is_err());
        assert!(path.join(".selection-enabled.json").exists());
        assert!(!path.join(DIRECTORY).exists());
        Ok(())
    })?;
    let path = temporary.path().to_owned();
    worker::run_reserved(TRUST_RESERVATION_BYTES, move |context, _| {
        let record = path.join(".selection-state/.substitute-selection.json");
        let original = std::fs::read(&record)?;
        let session = InstallationSession::create(choices(&path), context)?;
        let trust = session.update_trust([70; 32], &root(&key(70)?, 1, None)?, context)?;
        assert_eq!(&trust.installation, InstallationRecord::decode(&original)?.installation());
        assert_eq!(std::fs::read(record)?, original);
        Ok(())
    })
}
