// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real filesystem and shell-link publication, using inert accepted launcher payloads.
use super::*;
use sugarsubstitute_contracts::application_launch::{CURRENT_CONTRACT, CURRENT_LAUNCHER_CONTRACT};

const LAUNCHER: &str = "sugarsubstitute-launcher.exe";
const OWNERSHIP: &str = ".selection-state/.launcher-generation.json";
const LINK: &str = "SugarSubstitute.lnk";
mod updates;

/// Accept an inert package through the real trust/storage path and retain application custody.
fn prepare(
    root: &Directory,
    context: &JobContext,
) -> Result<crate::setup_storage::AcceptedApplication, Box<dyn std::error::Error + Send + Sync>> {
    accept_distribution(
        root,
        test_package::distribution_with_launcher(
            Some(CURRENT_CONTRACT),
            Some(CURRENT_LAUNCHER_CONTRACT),
        )?,
        context,
    )?;
    let store = Arc::new(SelectionStore::open(&root.path, context)?);
    let mut run = InstallationRun::acquire(store, context)?;
    let InstallationLaunch::Application(application) = run.select_launch(None, context)? else {
        return Err("Accepted launcher fixture absent".into());
    };
    Ok(*application)
}

#[test]
fn accepted_launcher_and_shortcut_publish_under_captured_directory_pin() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let application = prepare(&root, context)?;
        use std::os::windows::fs::OpenOptionsExt;
        use windows::Win32::Storage::FileSystem::{
            FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
        };
        let _capture_parent = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ.0)
            .custom_flags((FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT).0)
            .open(&root.path)?;
        let selection = root.path.join(".selection-state/.substitute-selection.json");
        let before = std::fs::read(&selection)?;
        let old_link = std::fs::read(root.path.join(LINK))?;
        let installed = application.repair_launch_entry(context)?;
        assert_eq!(installed.executable, root.path.join(LAUNCHER));
        assert_eq!(installed.directory, root.path);
        assert_eq!(
            std::fs::read(root.path.join(LAUNCHER))?,
            std::fs::read(application.directory().join(LAUNCHER))?
        );
        let stable = std::fs::read(root.path.join(LINK))?;
        assert_ne!(stable, old_link);
        let record = std::fs::read(root.path.join(OWNERSHIP))?;
        let ownership: serde_json::Value = serde_json::from_slice(&record)?;
        assert_eq!(ownership["source"]["id"], application.generation().id.as_str());
        assert_eq!(ownership["source"]["package"], application.generation().package.as_str());
        assert_eq!(entrypoint::native::target(&root.path.join(LINK))?, root.path.join(LAUNCHER));
        let link = root.read(LINK)?;
        let identity = crate::setup_storage::files::file_identity(&link)?;
        application.repair_launch_entry(context)?;
        assert_eq!(crate::setup_storage::files::file_identity(&root.read(LINK)?)?, identity);
        assert_eq!(std::fs::read(root.path.join(LINK))?, stable);
        assert_eq!(std::fs::read(root.path.join(OWNERSHIP))?, record);
        assert_eq!(std::fs::read(selection)?, before);
        assert_eq!(
            std::fs::read(root.path.join("releases/installed/SugarSubstitute.lnk"))?,
            old_link
        );
        Ok(())
    })
}

#[test]
fn persisted_launcher_intent_repairs_missing_image_and_links_without_cache() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let application = prepare(&root, context)?;
        application.repair_launch_entry(context)?;
        let record = std::fs::read(root.path.join(OWNERSHIP))?;
        let image = std::fs::read(root.path.join(LAUNCHER))?;
        std::fs::remove_file(root.path.join(LINK))?;
        std::fs::remove_file(root.path.join(LAUNCHER))?;
        std::fs::remove_file(root.path.join(".selection-state/SugarSubstitute.lnk"))?;
        // A complete ownership intent without its published payload is a restart state.
        application.repair_launch_entry(context)?;
        assert_eq!(std::fs::read(root.path.join(OWNERSHIP))?, record);
        assert_eq!(std::fs::read(root.path.join(LAUNCHER))?, image);
        let link = std::fs::read(root.path.join(LINK))?;
        std::fs::remove_file(root.path.join(LINK))?;
        application.repair_launch_entry(context)?;
        assert_eq!(std::fs::read(root.path.join(LINK))?, link);
        Ok(())
    })
}

#[test]
fn foreign_root_executable_and_edited_links_cannot_create_launcher_ownership() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        for leaf in [LAUNCHER, LINK] {
            let temporary = tempfile::tempdir()?;
            let root = Directory::open(temporary.path(), false)?;
            let application = prepare(&root, context)?;
            std::fs::write(root.path.join(leaf), b"user-owned bytes")?;
            let link = std::fs::read(root.path.join(LINK))?;
            assert!(application.repair_launch_entry(context).is_err());
            assert_eq!(std::fs::read(root.path.join(leaf))?, b"user-owned bytes");
            assert_eq!(std::fs::read(root.path.join(LINK))?, link);
            assert!(!root.path.join(OWNERSHIP).exists());
        }
        Ok(())
    })
}

/// Give each independent tampering scenario its own unchanged native observation bound.
fn changed_stable_entry_case(leaf: &'static str) -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, move |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let application = prepare(&root, context)?;
        application.repair_launch_entry(context)?;
        let mut changed = std::fs::read(root.path.join(leaf))?;
        changed.extend_from_slice(b"modified");
        std::fs::write(root.path.join(leaf), &changed)?;
        assert!(application.repair_launch_entry(context).is_err());
        assert_eq!(std::fs::read(root.path.join(leaf))?, changed);
        Ok(())
    })
}

#[test]
fn changed_stable_payload_is_preserved() -> TestResult {
    changed_stable_entry_case(LAUNCHER)
}

#[test]
fn changed_stable_ownership_record_is_preserved() -> TestResult {
    changed_stable_entry_case(OWNERSHIP)
}

#[test]
fn changed_stable_shortcut_is_preserved() -> TestResult {
    changed_stable_entry_case(LINK)
}

#[test]
fn launcher_publication_requires_setup_custody_and_respects_cancellation() -> TestResult {
    worker::run_reserved_expiring(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, clock| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let application = prepare(&root, context)?;
        let before = std::fs::read(root.path.join(LINK))?;
        let admission =
            crate::setup_storage::transaction::admission::Admission::open(&root.path, false)?;
        assert!(application.repair_launch_entry(context).is_err());
        drop(admission);
        assert!(!root.path.join(OWNERSHIP).exists());
        clock.expire();
        assert!(application.repair_launch_entry(context).is_err());
        assert_eq!(std::fs::read(root.path.join(LINK))?, before);
        assert!(!root.path.join(OWNERSHIP).exists());
        assert!(!root.path.join(LAUNCHER).exists());
        Ok(())
    })
}

#[test]
fn missing_launcher_ownership_is_not_inferred_from_a_surviving_shell_entry() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let application = prepare(&root, context)?;
        application.repair_launch_entry(context)?;
        let link = std::fs::read(root.path.join(LINK))?;
        std::fs::remove_file(root.path.join(OWNERSHIP))?;
        std::fs::remove_file(root.path.join(LAUNCHER))?;
        assert!(application.repair_launch_entry(context).is_err());
        assert_eq!(std::fs::read(root.path.join(LINK))?, link);
        assert!(!root.path.join(OWNERSHIP).exists());
        assert!(!root.path.join(LAUNCHER).exists());
        Ok(())
    })
}

#[test]
fn matching_installer_publishes_and_returns_the_stable_launcher() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        drop(prepare(&root, context)?);
        let workspace = tempfile::tempdir()?;
        let (manifest, compressed) = test_package::distribution_with_launcher(
            Some(CURRENT_CONTRACT),
            Some(CURRENT_LAUNCHER_CONTRACT),
        )?;
        let archive = workspace.path().join("installer.exe");
        std::fs::write(&archive, test_package::seal(&manifest, &compressed))?;
        let digest = format!("{:x}", Sha256::digest(&manifest));
        let extracted = crate::setup_storage::materialize(
            &archive,
            &crate::setup_storage::cache::RuntimeCache::open(
                workspace.path(),
                crate::setup_storage::cache::CachePolicy::default(),
                context,
            )?,
            &digest,
            context,
        )?;
        let bundle = Arc::new(extracted);
        let prepared = crate::setup_storage::InstallationSession::maintain(&root.path, context)?
            .prepare(bundle, context)?;
        prepared.stage(context)?;
        let installed =
            prepared.activate(prepared.maintenance_runtime().ok_or("Runtime absent")?, context)?;
        assert_eq!(installed.executable, root.path.join(LAUNCHER));
        assert_eq!(installed.directory, root.path);
        assert_eq!(entrypoint::native::target(&root.path.join(LINK))?, installed.executable);
        assert!(!root.path.join("external-comfy").exists());
        assert!(!root.path.join("python.exe").exists());
        Ok(())
    })
}
