// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reuse immutable preview custody only after current signed archive authentication.
use super::*;

/// Exercise the producing installer's initial extraction and complete payload verification.
fn preview(fixture: &Fixture, context: &JobContext) -> Result<Arc<VerifiedBundle>, Failure> {
    let distribution =
        InstallerDistribution::decode(&serde_json::to_vec(&fixture.configuration)?, context)?;
    let root = crate::setup_storage::materialize(
        &fixture.executable,
        &crate::setup_storage::cache::RuntimeCache::open(
            fixture.workspace.path(),
            crate::setup_storage::cache::CachePolicy::default(),
            context,
        )?,
        distribution.manifest_digest(),
        context,
    )?;
    Ok(Arc::new(root))
}

/// Supply retained bytes to the actual signed import without weakening its trust checks.
fn import(
    fixture: &Fixture,
    preview: Arc<VerifiedBundle>,
    now: i64,
    context: &JobContext,
) -> Result<SignedInstallation, Failure> {
    let distribution =
        InstallerDistribution::decode(&serde_json::to_vec(&fixture.configuration)?, context)?;
    Ok(distribution.import(
        InstallationSession::maintain(fixture.root.path(), context)?,
        &fixture.executable,
        BundleSource::Retained(preview),
        || now,
        context,
    )?)
}

#[test]
fn imported_payload_keeps_source_files_immutable_after_preview_owner_retires() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let preview = preview(&fixture, context)?;
        let worker = preview.worker()?.executable().to_owned();
        let original = std::fs::read(&worker)?;
        let imported = import(&fixture, preview, NOW, context)?;
        assert!(std::fs::write(&worker, b"changed while retained").is_err());
        assert_eq!(std::fs::read(&worker)?, original);
        assert_eq!(
            std::fs::read_dir(
                fixture
                    .workspace
                    .path()
                    .join(crate::setup_storage::cache::DIRECTORY)
                    .join(crate::setup_storage::cache::Namespace::PackedPayloads.directory())
            )?
            .count(),
            1
        );
        drop(imported);
        std::fs::write(&worker, b"released after import retired")?;
        Ok(())
    })
}

#[test]
fn retained_preview_cannot_replace_archive_byte_authentication() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let preview = preview(&fixture, context)?;
        let mut bytes = std::fs::read(&fixture.executable)?;
        bytes[b"inert installer prefix".len()] ^= 1;
        std::fs::write(&fixture.executable, bytes)?;
        assert!(import(&fixture, preview, NOW, context).is_err());
        assert_eq!(
            std::fs::read_dir(
                fixture
                    .workspace
                    .path()
                    .join(crate::setup_storage::cache::DIRECTORY)
                    .join(crate::setup_storage::cache::Namespace::PackedPayloads.directory())
            )?
            .count(),
            1
        );
        assert!(
            SelectionStore::open(fixture.root.path(), context)?
                .load(context)?
                .ok_or("Selection absent")?
                .candidate()
                .is_none()
        );
        Ok(())
    })
}

#[test]
fn retained_preview_cannot_replace_current_metadata_authentication() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let mut fixture = Fixture::new(context)?;
        let preview = preview(&fixture, context)?;
        fixture.configuration["targets"] = serde_json::json!("{}");
        assert!(import(&fixture, preview, NOW, context).is_err());
        assert_eq!(
            std::fs::read_dir(
                fixture
                    .workspace
                    .path()
                    .join(crate::setup_storage::cache::DIRECTORY)
                    .join(crate::setup_storage::cache::Namespace::PackedPayloads.directory())
            )?
            .count(),
            1
        );
        assert!(
            SelectionStore::open(fixture.root.path(), context)?
                .load(context)?
                .ok_or("Selection absent")?
                .candidate()
                .is_none()
        );
        Ok(())
    })
}

#[test]
fn retained_preview_cannot_extend_expired_update_authority() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let preview = preview(&fixture, context)?;
        assert!(import(&fixture, preview, EXPIRES, context).is_err());
        assert_eq!(
            std::fs::read_dir(
                fixture
                    .workspace
                    .path()
                    .join(crate::setup_storage::cache::DIRECTORY)
                    .join(crate::setup_storage::cache::Namespace::PackedPayloads.directory())
            )?
            .count(),
            1
        );
        assert!(
            SelectionStore::open(fixture.root.path(), context)?
                .load(context)?
                .ok_or("Selection absent")?
                .candidate()
                .is_none()
        );
        Ok(())
    })
}

#[test]
fn another_valid_preview_cannot_supply_a_different_signed_inventory() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let other = tempfile::tempdir()?;
        let (manifest, compressed) = test_package::distribution_with_launcher(
            Some(sugarsubstitute_contracts::application_launch::CURRENT_CONTRACT),
            Some(sugarsubstitute_contracts::application_launch::CURRENT_LAUNCHER_CONTRACT),
        )?;
        let archive = test_package::seal(&manifest, &compressed);
        let source = other.path().join("other.ssupdate");
        std::fs::write(&source, archive)?;
        let digest = format!("{:x}", Sha256::digest(&manifest));
        let cache = crate::setup_storage::cache::RuntimeCache::open(
            other.path(),
            crate::setup_storage::cache::CachePolicy::default(),
            context,
        )?;
        let preview =
            Arc::new(crate::setup_storage::materialize(&source, &cache, &digest, context)?);
        let worker = preview.worker()?.executable().to_owned();
        let before = std::fs::read(&worker)?;
        assert!(import(&fixture, preview, NOW, context).is_err());
        assert_eq!(std::fs::read(&worker)?, before);
        assert!(
            !fixture
                .workspace
                .path()
                .join(crate::setup_storage::cache::DIRECTORY)
                .join(crate::setup_storage::cache::Namespace::PackedPayloads.directory())
                .exists()
        );
        assert!(
            SelectionStore::open(fixture.root.path(), context)?
                .load(context)?
                .ok_or("Selection absent")?
                .candidate()
                .is_none()
        );
        Ok(())
    })
}
