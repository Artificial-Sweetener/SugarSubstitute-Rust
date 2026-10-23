// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Acquisition precedes signed delivery and transfers its native exclusion without reacquisition.
use super::*;
use crate::setup_storage::{
    Download, InstallationRun, SelectionStore, TRUST_RESERVATION_BYTES,
    existing::fixture,
    test_package::{self, repository::SignedPackage},
    test_worker::{self as worker, TestResult},
};
use sugarsubstitute_update::fixture::NOW;

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
fn fresh_session_reserves_no_generation_and_releases_exclusion_on_drop() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let root = temporary.path().join("installation");
        let session = InstallationSession::create(choices(&root), context)?;
        assert_eq!(session.directory(), root);
        assert!(!root.join("releases").exists());
        assert!(!root.join("external-comfy").exists());
        assert!(InstallationSession::create(choices(&root), context).is_err());
        drop(session);
        let reopened = InstallationSession::create(choices(&root), context)?;
        assert_eq!(reopened.directory(), root);
        assert!(!root.join("releases").exists());
        Ok(())
    })
}

#[test]
fn maintenance_retains_saved_inputs_before_delivery_and_preserves_them_after_release() -> TestResult
{
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        let original = fixture::installed(&directory, context)?;
        let saved = std::fs::read(original.join("startup.local.toml"))?;
        let session = InstallationSession::maintain(directory.path(), context)?;
        assert_eq!(
            session.existing.as_ref().ok_or("Saved state absent")?.summary.selection.locale,
            "ja"
        );
        assert!(std::fs::write(original.join("startup.local.toml"), b"replace").is_err());
        assert!(InstallationSession::maintain(directory.path(), context).is_err());
        assert_eq!(std::fs::read_dir(directory.path().join("releases"))?.count(), 1);
        drop(session);
        let reopened = InstallationSession::maintain(directory.path(), context)?;
        assert_eq!(reopened.directory(), directory.path());
        assert_eq!(std::fs::read(original.join("startup.local.toml"))?, saved);
        assert!(!directory.path().join("external-comfy").exists());
        Ok(())
    })
}

#[test]
fn unknown_installation_files_cannot_be_adopted_or_changed_by_session_acquisition() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("user-document");
        std::fs::write(&path, b"preserve")?;
        assert!(InstallationSession::create(choices(temporary.path()), context).is_err());
        assert!(InstallationSession::maintain(temporary.path(), context).is_err());
        assert_eq!(std::fs::read(path)?, b"preserve");
        assert_eq!(std::fs::read_dir(temporary.path())?.count(), 1);
        Ok(())
    })
}

#[test]
fn signed_delivery_prepares_under_the_original_session_while_run_custody_remains_live() -> TestResult
{
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let workspace = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        let original = fixture::installed(&directory, context)?;
        let saved = std::fs::read(original.join("startup.local.toml"))?;
        let session = InstallationSession::maintain(directory.path(), context)?;
        let (manifest, compressed) = test_package::distribution()?;
        let archive = test_package::seal(&manifest, &compressed);
        let signed = SignedPackage::new(&archive, 1)?;
        let trust = session.update_trust([92; 32], &signed.root, context)?;
        let selection = Arc::new(SelectionStore::open(session.directory(), context)?);
        let running = InstallationRun::acquire(selection.clone(), context)?;
        let before = selection.load(context)?.ok_or("Selection absent")?;
        let target = trust
            .offer(signed.metadata.metadata(), &signed.request, &signed.descriptor, NOW, context)?
            .into_package()?;
        let source = workspace.path().join("setup.exe");
        std::fs::write(&source, [b"inert setup prefix".as_slice(), &archive].concat())?;
        let download = Download::import_installer(
            trust.runtime_cache(context)?,
            target,
            &source,
            || NOW,
            context,
        )?;
        let bundle = Arc::new(VerifiedBundle::from_download(
            download,
            &crate::setup_storage::cache::RuntimeCache::open(
                workspace.path(),
                crate::setup_storage::cache::CachePolicy::default(),
                context,
            )?,
            context,
        )?);
        assert!(InstallationSession::maintain(directory.path(), context).is_err());
        let prepared = session.prepare(bundle, context)?;
        assert!(InstallationSession::maintain(directory.path(), context).is_err());
        let candidate = prepared.prepare_update(
            prepared.maintenance_runtime().ok_or("Saved runtime absent")?,
            &signed.authorization(&trust),
            || NOW,
            context,
        )?;
        assert_eq!(
            selection.load(context)?.ok_or("Selection absent")?.accepted(),
            before.accepted()
        );
        assert_eq!(std::fs::read(candidate.directory().join("startup.local.toml"))?, saved);
        assert_eq!(std::fs::read(original.join("startup.local.toml"))?, saved);
        assert!(!directory.path().join("external-comfy").exists());
        drop(prepared);
        let reopened = InstallationSession::maintain(directory.path(), context)?;
        assert_eq!(reopened.directory(), directory.path());
        assert!(InstallationRun::acquire(selection, context).is_err());
        drop(running);
        Ok(())
    })
}
