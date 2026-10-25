// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Maintenance preserves accepted configuration independently of old shortcuts and broken payloads.
use super::*;
use crate::setup_storage::{existing::ExistingInstallation, inspect_installation};

/// Supply the same pinned-manifest boundary as an embedded installer, with inert bytes only.
fn embedded_bundle(
    workspace: &Path,
    replacement: bool,
    context: &JobContext,
) -> Result<Arc<VerifiedBundle>, Box<dyn std::error::Error + Send + Sync>> {
    let (manifest, compressed) = test_package::distribution()?;
    let mut value: serde_json::Value = serde_json::from_slice(&manifest)?;
    if replacement {
        value["release_id"] = "replacement-release".into();
    }
    let manifest = serde_json::to_vec(&value)?;
    let archive = workspace.join("installer.exe");
    std::fs::write(&archive, test_package::seal(&manifest, &compressed))?;
    let digest = format!("{:x}", Sha256::digest(&manifest));
    let extracted = crate::setup_storage::materialize(
        &archive,
        &crate::setup_storage::cache::RuntimeCache::open(
            workspace,
            crate::setup_storage::cache::CachePolicy::default(),
            context,
        )?,
        &digest,
        context,
    )?;
    Ok(Arc::new(extracted))
}

#[test]
fn identical_embedded_installer_repairs_access_to_the_accepted_generation() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        for missing in [false, true] {
            let temporary = tempfile::tempdir()?;
            let root = Directory::open(temporary.path(), false)?;
            let (generation, _) = accept_fixture(&root, context)?;
            let link = root.path().join("SugarSubstitute.lnk");
            if missing {
                std::fs::remove_file(&link)?;
            }
            let workspace = tempfile::tempdir()?;
            let bundle = embedded_bundle(workspace.path(), false, context)?;
            let prepared =
                crate::setup_storage::InstallationSession::maintain(root.path(), context)?
                    .prepare(bundle, context)?;
            prepared.stage(context)?;
            let store = SelectionStore::open(root.path(), context)?;
            let before = store.load(context)?;
            let installed = prepared
                .activate(prepared.maintenance_runtime().ok_or("Runtime absent")?, context)?;
            let accepted = root.path().join("releases").join(generation.id.as_str());
            assert_eq!(installed.directory, accepted);
            assert_eq!(installed.executable, accepted.join("sugarsubstitute.exe"));
            assert_eq!(
                entrypoint::selected(&root, context)?.ok_or("Repaired entry absent")?.path(),
                accepted
            );
            assert_eq!(store.load(context)?, before);
            assert!(!root.path().join("external-comfy").exists());
            assert!(!root.path().join("python.exe").exists());
        }
        Ok(())
    })
}

#[test]
fn matching_installer_cannot_treat_missing_or_changed_accepted_payloads_as_healthy() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        for missing in [false, true] {
            let temporary = tempfile::tempdir()?;
            let root = Directory::open(temporary.path(), false)?;
            let (generation, _) = accept_fixture(&root, context)?;
            let executable = root
                .path()
                .join("releases")
                .join(generation.id.as_str())
                .join("sugarsubstitute.exe");
            if missing {
                std::fs::remove_file(&executable)?;
            } else {
                std::fs::write(&executable, b"corrupt payload")?;
            }
            let workspace = tempfile::tempdir()?;
            let bundle = embedded_bundle(workspace.path(), false, context)?;
            let prepared =
                crate::setup_storage::InstallationSession::maintain(root.path(), context)?
                    .prepare(bundle, context)?;
            prepared.stage(context)?;
            let link = root.path().join("SugarSubstitute.lnk");
            let before = std::fs::read(&link)?;
            assert!(
                prepared
                    .activate(prepared.maintenance_runtime().ok_or("Runtime absent")?, context)
                    .is_err()
            );
            assert_eq!(std::fs::read(&link)?, before);
            if missing {
                assert!(!executable.exists());
            } else {
                assert_eq!(std::fs::read(&executable)?, b"corrupt payload");
            }
        }
        Ok(())
    })
}

#[test]
fn embedded_installer_cannot_replace_an_accepted_launch_with_an_unaccepted_generation() -> TestResult
{
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        accept_fixture(&root, context)?;
        let workspace = tempfile::tempdir()?;
        let bundle = embedded_bundle(workspace.path(), true, context)?;
        let prepared = crate::setup_storage::InstallationSession::maintain(root.path(), context)?
            .prepare(bundle, context)?;
        prepared.stage(context)?;
        let store = SelectionStore::open(root.path(), context)?;
        let before = store.load(context)?;
        let link = root.path().join("SugarSubstitute.lnk");
        let old_link = std::fs::read(&link)?;
        assert!(
            prepared
                .activate(prepared.maintenance_runtime().ok_or("Runtime absent")?, context,)
                .is_err()
        );
        assert_eq!(std::fs::read(&link)?, old_link);
        assert_eq!(store.load(context)?, before);
        Ok(())
    })
}

#[test]
fn maintenance_uses_accepted_configuration_and_can_repair_its_missing_executable() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let (generation, _) = accept_fixture(&root, context)?;
        let accepted = root.path().join("releases").join(generation.id.as_str());
        let current = b"schema_version = 1\n# Accepted user customization\n";
        std::fs::write(accepted.join("startup.local.toml"), current)?;
        let link = root.path().join("SugarSubstitute.lnk");
        let old_link = std::fs::read(&link)?;
        assert_ne!(
            entrypoint::selected(&root, context)?.ok_or("Old shell selection absent")?.path(),
            accepted
        );
        std::fs::remove_file(accepted.join("sugarsubstitute.exe"))?;
        let summary = inspect_installation(root.path(), context)?.ok_or("Maintenance absent")?;
        assert!(summary.executable_missing);
        assert!(!summary.launch_entry_missing);
        let existing = ExistingInstallation::read(&root, context)?.ok_or("Settings absent")?;
        let replacement = Directory::open(&root.path().join("replacement"), true)?;
        existing.write_configuration(replacement.path(), context)?;
        assert_eq!(std::fs::read(replacement.path().join("startup.local.toml"))?, current);
        assert_eq!(std::fs::read(&link)?, old_link);
        std::fs::remove_file(&link)?;
        assert!(
            inspect_installation(root.path(), context)?
                .ok_or("Repair absent")?
                .launch_entry_missing
        );
        assert!(!link.exists());
        Ok(())
    })
}

#[test]
fn unknown_shell_bytes_are_preserved_and_do_not_choose_maintenance_configuration() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        accept_fixture(&root, context)?;
        let link = root.path().join("SugarSubstitute.lnk");
        std::fs::write(&link, b"user-owned bytes")?;
        assert!(inspect_installation(root.path(), context)?.is_some());
        assert_eq!(std::fs::read(&link)?, b"user-owned bytes");
        Ok(())
    })
}

#[test]
fn damaged_accepted_configuration_does_not_fall_back_to_the_old_shell_target() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        for mismatch in [false, true] {
            let temporary = tempfile::tempdir()?;
            let root = Directory::open(temporary.path(), false)?;
            let (generation, _) = accept_fixture(&root, context)?;
            let path = root
                .path()
                .join("releases")
                .join(generation.id.as_str())
                .join("installation-ready.json");
            let bytes = if mismatch {
                let mut record: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
                record["bundle_sha256"] = serde_json::Value::String("f".repeat(64));
                serde_json::to_vec(&record)?
            } else {
                b"broken saved configuration".to_vec()
            };
            std::fs::write(&path, &bytes)?;
            assert!(entrypoint::selected(&root, context)?.is_some());
            assert!(inspect_installation(root.path(), context).is_err());
            assert_eq!(std::fs::read(&path)?, bytes);
        }
        Ok(())
    })
}

#[test]
fn damaged_or_missing_selection_never_becomes_legacy_shortcut_recovery() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        accept_fixture(&root, context)?;
        let path = root.path().join(".selection-state/.substitute-selection.json");
        std::fs::write(&path, b"damaged selection")?;
        assert!(inspect_installation(root.path(), context).is_err());
        assert_eq!(std::fs::read(&path)?, b"damaged selection");
        std::fs::remove_file(&path)?;
        assert!(inspect_installation(root.path(), context).is_err());
        assert!(!path.exists());
        assert!(entrypoint::selected(&root, context)?.is_some());
        Ok(())
    })
}

#[test]
fn configuration_captured_before_a_new_acceptance_cannot_populate_another_generation() -> TestResult
{
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        accept_fixture(&root, context)?;
        let existing = ExistingInstallation::read(&root, context)?.ok_or("Settings absent")?;
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        let before = store.load(context)?.ok_or("Selection absent")?;
        let generation = Generation {
            id: GenerationId::new("2".repeat(32))?,
            package: PackageDigest::new("c".repeat(64))?,
        };
        let staging = before.prepare(generation.clone(), PreparationIntent::Automatic)?;
        store.compare_exchange(&before, &staging, context)?;
        let prepared = staging.prepared(&generation)?;
        store.compare_exchange(&staging, &prepared, context)?;
        let mut owner = InstallationRun::acquire(store, context)?;
        let trial = owner.begin_trial(&prepared, context)?;
        owner.accept(&trial, context)?;
        let output = Directory::open(&root.path().join("replacement"), true)?;
        assert!(existing.write_configuration(output.path(), context).is_err());
        assert!(std::fs::read_dir(output.path())?.next().is_none());
        Ok(())
    })
}
