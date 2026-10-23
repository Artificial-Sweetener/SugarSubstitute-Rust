// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticated archive custody feeds actual native maintenance without provisioning.
use super::*;
use crate::setup_storage::{
    Download, PreparedInstallation, entrypoint, existing::fixture, test_worker::installation,
};
use crate::setup_storage::{
    TRUST_RESERVATION_BYTES, TrustStore, test_package::repository::SignedPackage,
};
use std::sync::Arc;
use sugarsubstitute_update::{RepositoryTrust, delivery::Response, fixture::*};

fn verified(
    bytes: &[u8],
    installation_root: &Path,
    context: &JobContext,
) -> Result<VerifiedDownload, Box<dyn std::error::Error + Send + Sync>> {
    let key = key(92)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    trust.refresh(Set::new(&key, [1; 3], bytes, EXPIRES)?.metadata(), NOW)?;
    receive(bytes, installation_root, trust.target(NAME, NOW)?, context)
}
fn receive(
    bytes: &[u8],
    installation_root: &Path,
    target: sugarsubstitute_update::Target,
    context: &JobContext,
) -> Result<VerifiedDownload, Box<dyn std::error::Error + Send + Sync>> {
    let mut download = Download::open(
        crate::setup_storage::cache::RuntimeCache::open(
            installation_root,
            crate::setup_storage::cache::CachePolicy::default(),
            context,
        )?,
        target,
        context,
    )?;
    download.receive(
        Response { status: 200, length: Some(bytes.len() as u64), range: None, encoding: None },
        &mut io::Cursor::new(bytes),
        context,
    )?;
    Ok(download.finish(|| NOW, context)?)
}

/// Qualify one independent maintenance scenario within its own unchanged execution deadline.
fn downloaded_candidate_case(missing: bool, missing_entry: bool) -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 48 * 1024 * 1024, move |context, _| {
        let source_root = installation(92, context)?;
        let workspace = tempfile::tempdir()?;
        let (manifest, compressed) = test_package::distribution()?;
        let archive = test_package::seal(&manifest, &compressed);
        let target = tempfile::tempdir()?;
        let root = Directory::open(target.path(), false)?;
        let original = fixture::installed(&root, context)?;
        let signed = SignedPackage::new(&archive, 1)?;
        let trust = signed.initialize(&root.path, context)?;
        let offer = trust.offer(
            signed.metadata.metadata(),
            &signed.request,
            &signed.descriptor,
            NOW,
            context,
        )?;
        let bundle = Arc::new(VerifiedBundle::from_download(
            receive(&archive, source_root.path(), offer.into_package()?, context)?,
            &crate::setup_storage::cache::RuntimeCache::open(
                workspace.path(),
                crate::setup_storage::cache::CachePolicy::default(),
                context,
            )?,
            context,
        )?);
        assert_eq!(
            bundle.download_identity().ok_or("Missing archive identity")?.digest(),
            <[u8; 32]>::from(Sha256::digest(&archive))
        );
        if missing {
            std::fs::remove_file(original.join("sugarsubstitute.exe"))?;
        }
        if missing_entry {
            std::fs::remove_file(root.path.join("SugarSubstitute.lnk"))?;
        }
        let saved = std::fs::read(original.join("startup.local.toml"))?;
        std::fs::write(root.path.join("user-document"), b"authored data")?;
        let prepared = crate::setup_storage::InstallationSession::maintain(&root.path, context)?
            .prepare(bundle.clone(), context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Maintenance runtime absent")?.clone();
        prepared.stage(context)?;
        assert_eq!(
            entrypoint::selected(&root)?.map(|directory| directory.path),
            if missing_entry { None } else { Some(original.clone()) }
        );
        assert!(prepared.activate(&runtime, context).is_err());
        let reopened = TrustStore::open(&root.path, [92; 32], context)?;
        let installed =
            prepared.prepare_update(&runtime, &signed.authorization(&reopened), || NOW, context)?;
        assert_ne!(installed.directory(), original);
        assert_eq!(std::fs::read(installed.directory().join("startup.local.toml"))?, saved);
        assert_eq!(std::fs::read(original.join("startup.local.toml"))?, saved);
        assert_eq!(std::fs::read(root.path.join("user-document"))?, b"authored data");
        assert_eq!(
            entrypoint::selected(&root)?.map(|directory| directory.path),
            if missing_entry { None } else { Some(original.clone()) }
        );
        let store = crate::setup_storage::SelectionStore::open(&root.path, context)?;
        let state = store.load(context)?.ok_or("Candidate selection absent")?;
        assert!(state.accepted().is_none());
        assert!(matches!(state.candidate(),
            Some(sugarsubstitute_contracts::installation::Candidate::Prepared { generation })
                if generation == installed.generation()));
        assert_eq!(
            installed.directory(),
            root.path.join("releases").join(installed.generation().id.as_str())
        );
        assert_eq!(installed.generation().package.as_str(), bundle.digest());
        assert!(std::fs::rename(installed.directory(), root.path.join("moved")).is_err());
        assert!(!runtime.directory.exists());
        assert!(!runtime.python.exists());
        assert!(std::fs::write(&bundle.worker_paths().0, b"tampered").is_err());
        assert!(verified(&archive, source_root.path(), context).is_err());
        Ok(())
    })
}

#[test]
fn downloaded_candidate_preserves_complete_installation() -> TestResult {
    downloaded_candidate_case(false, false)
}

#[test]
fn downloaded_candidate_preserves_missing_application() -> TestResult {
    downloaded_candidate_case(true, false)
}

#[test]
fn downloaded_candidate_preserves_missing_shortcut() -> TestResult {
    downloaded_candidate_case(false, true)
}

#[test]
fn downloaded_candidate_preserves_missing_application_and_shortcut() -> TestResult {
    downloaded_candidate_case(true, true)
}

#[test]
fn signed_but_malformed_archive_cannot_return_a_bundle_or_delete_saved_files() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let source_root = installation(93, context)?;
        let workspace = tempfile::tempdir()?;
        std::fs::write(workspace.path().join("preserved"), b"original")?;
        let (manifest, mut compressed) = test_package::distribution()?;
        compressed.extend_from_slice(b"extra");
        let archive = test_package::seal(&manifest, &compressed);
        assert!(
            VerifiedBundle::from_download(
                verified(&archive, source_root.path(), context)?,
                &crate::setup_storage::cache::RuntimeCache::open(
                    workspace.path(),
                    crate::setup_storage::cache::CachePolicy::default(),
                    context
                )?,
                context
            )
            .is_err()
        );
        let mut prefixed = b"MZunexpected prefix".to_vec();
        prefixed.extend(test_package::seal(&manifest, &compressed));
        assert!(
            VerifiedBundle::from_download(
                verified(&prefixed, source_root.path(), context)?,
                &crate::setup_storage::cache::RuntimeCache::open(
                    workspace.path(),
                    crate::setup_storage::cache::CachePolicy::default(),
                    context
                )?,
                context
            )
            .is_err()
        );
        assert_eq!(std::fs::read(workspace.path().join("preserved"))?, b"original");
        Ok(())
    })
}

mod authorization;
