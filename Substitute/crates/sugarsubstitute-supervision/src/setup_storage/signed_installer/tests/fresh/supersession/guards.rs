// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recovery recognizes unfinished native metadata without treating arbitrary content as fresh setup.
use super::*;

#[test]
fn completed_unaccepted_settings_require_maintenance_after_supersession() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path();
        let root = path.join("native");
        let runtime = runtime(path)?;
        let first = version(path, 1, context)?.prepare(|| NOW, context)?.complete(
            &runtime,
            || NOW,
            context,
        )?;
        let original = first.directory().to_owned();
        drop(first);
        let mut configuration = std::fs::read(original.join("startup.local.toml"))?;
        configuration.extend_from_slice(b"\n# User customization preserved across setup retry\n");
        std::fs::write(original.join("startup.local.toml"), &configuration)?;
        let session = InstallationSession::maintain(&root, context)?;
        drop(import_version(session, path, 2, context)?.prepare(|| NOW, context)?);
        let store = SelectionStore::open(&root, context)?;
        let before = store.load(context)?;
        assert!(inspect_installation(&root, context)?.is_some());
        assert!(InstallationSession::create(choices(path), context).is_err());
        assert_eq!(store.load(context)?, before);
        let session = InstallationSession::maintain(&root, context)?;
        let prepared = import_version(session, path, 2, context)?.prepare(|| NOW, context)?;
        assert_eq!(prepared.maintenance_runtime(), Some(&runtime));
        let replacement = prepared.complete(&runtime, || NOW, context)?;
        assert_eq!(
            std::fs::read(replacement.directory().join("startup.local.toml"))?,
            configuration
        );
        assert_eq!(std::fs::read(original.join("startup.local.toml"))?, configuration);
        assert!(store.load(context)?.ok_or("Selection absent")?.accepted().is_none());
        Ok(())
    })
}

#[test]
fn unfinished_predecessors_require_valid_metadata_and_never_hide_completed_configuration()
-> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path();
        let runtime = runtime(path)?;
        drop(version(path, 1, context)?.prepare(|| NOW, context)?);
        let store = SelectionStore::open(&path.join("native"), context)?;
        let first = store.load(context)?.ok_or("Selection absent")?;
        let previous = path
            .join("native/releases")
            .join(first.candidate().ok_or("Candidate absent")?.generation().id.as_str());
        drop(version(path, 2, context)?.prepare(|| NOW, context)?);
        let before = store.load(context)?;
        let marker = previous.join("setup-generation.json");
        let original = std::fs::read(&marker)?;
        for field in ["schema_version", "bundle_sha256", "state", "unknown"] {
            let mut changed: serde_json::Value = serde_json::from_slice(&original)?;
            changed[field] = "unrecognized".into();
            let changed = serde_json::to_vec(&changed)?;
            std::fs::write(&marker, &changed)?;
            assert!(InstallationSession::create(choices(path), context).is_err());
            assert_eq!(std::fs::read(&marker)?, changed);
            assert_eq!(store.load(context)?, before);
        }
        std::fs::write(&marker, &original)?;
        for completed in [previous.clone(), path.join("native/releases/.generation-init-preserved")]
        {
            std::fs::create_dir_all(&completed)?;
            let ready = completed.join("installation-ready.json");
            std::fs::write(&ready, b"damaged completed settings")?;
            assert!(InstallationSession::create(choices(path), context).is_err());
            assert_eq!(std::fs::read(&ready)?, b"damaged completed settings");
            assert_eq!(store.load(context)?, before);
            std::fs::remove_file(ready)?;
        }
        let unknown = path.join("native/releases").join("0".repeat(32));
        std::fs::create_dir(&unknown)?;
        let note = unknown.join("user-note");
        std::fs::write(&note, b"unknown directory")?;
        assert!(InstallationSession::create(choices(path), context).is_err());
        assert_eq!(std::fs::read(&note)?, b"unknown directory");
        assert_eq!(store.load(context)?, before);
        // Move the fixture out of discovery; recovery never changes or deletes unknown files.
        let moved = path.join("preserved-unknown");
        std::fs::rename(&unknown, &moved)?;
        drop(version(path, 2, context)?.prepare(|| NOW, context)?.complete(
            &runtime,
            || NOW,
            context,
        )?);
        assert_eq!(std::fs::read(moved.join("user-note"))?, b"unknown directory");
        Ok(())
    })
}
