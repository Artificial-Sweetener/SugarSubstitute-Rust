// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Installed engine facts remain distinct from running capabilities and live preparation custody.
use super::*;
use crate::setup_storage::{
    UpdaterPackage,
    test_package::updater::{accept_prepared, archive, evidence, run},
    test_worker::{self as worker, TestResult},
};
use sugarsubstitute_contracts::application_launch::updater::CURRENT_UPDATER_CONTRACT;
use sugarsubstitute_update::fixture::NOW;

fn prepared(
    root: &Path,
    workspace: &Path,
    context: &JobContext,
) -> Result<crate::setup_storage::PreparedUpdater, Box<dyn std::error::Error + Send + Sync>> {
    let (bytes, _) = archive(CURRENT_UPDATER_CONTRACT)?;
    let (trust, offer, download) = evidence(root, &bytes, Component::Updater, context)?;
    Ok(UpdaterPackage::import(root, trust, offer, download, workspace, || NOW, context)?
        .stage(|| NOW, context)?)
}

#[test]
fn healthy_engine_identity_never_changes_the_running_process_capabilities() -> TestResult {
    run(|context| {
        let root = worker::installation(171, context)?;
        let workspace = tempfile::tempdir()?;
        let prepared = prepared(root.path(), workspace.path(), context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        assert_eq!(trust.updater_request(Channel::Release, context)?.sequence_floor, 0);
        assert!(trust.pin_updater(Channel::Release, 2, context)?.is_none());
        accept_prepared(prepared, context)?;
        let request = trust.updater_request(Channel::Release, context)?;
        assert_eq!(request.component, Component::Updater);
        assert_eq!(request.sequence_floor, 2);
        assert!(request.sequence_digest.is_some());
        assert_eq!(request.updater_release, sugarsubstitute_update::ENGINE_RELEASE);
        assert_eq!(request.updater_protocol, sugarsubstitute_update::ENGINE_PROTOCOL);
        assert_eq!(request.package_schema, sugarsubstitute_update::package::SCHEMA_VERSION);
        assert_eq!(trust.updater_request(Channel::Canary, context)?.sequence_floor, 0);
        assert!(trust.pin_updater(Channel::Canary, 2, context)?.is_none());
        assert!(trust.pin_updater(Channel::Release, 3, context)?.is_none());
        let image =
            trust.pin_updater(Channel::Release, 2, context)?.ok_or("Healthy image absent")?;
        let executable = image.executable();
        assert!(std::fs::OpenOptions::new().write(true).open(&executable).is_err());
        drop(trust);
        let mut trust = TrustStore::open(root.path(), [92; 32], context)?;
        trust.acquire_preparation(context)?;
        drop(trust.claim_updater_preparation(context)?);
        drop(image);
        assert!(std::fs::OpenOptions::new().write(true).open(executable).is_ok());
        Ok(())
    })
}

#[test]
fn accepted_engine_reuse_rechecks_payload_and_preserves_damage() -> TestResult {
    run(|context| {
        let root = worker::installation(172, context)?;
        let workspace = tempfile::tempdir()?;
        accept_prepared(prepared(root.path(), workspace.path(), context)?, context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let image =
            trust.pin_updater(Channel::Release, 2, context)?.ok_or("Healthy image absent")?;
        let directory = image.directory().to_owned();
        drop(image);
        for role in
            ["sugarsubstitute-updater.exe", "vcruntime140.dll", "sugarsubstitute-updater.json"]
        {
            let path = directory.join("app").join(role);
            let original = std::fs::read(&path)?;
            std::fs::write(&path, b"damaged engine")?;
            assert!(trust.pin_updater(Channel::Release, 2, context).is_err(), "accepted {role}");
            assert_eq!(std::fs::read(&path)?, b"damaged engine");
            std::fs::write(path, original)?;
        }
        assert_eq!(trust.updater_request(Channel::Release, context)?.sequence_floor, 2);
        Ok(())
    })
}

#[test]
fn accepted_engine_reuse_never_opens_symbols_but_complete_verification_requires_them() -> TestResult
{
    use std::os::windows::fs::OpenOptionsExt;
    run(|context| {
        let root = worker::installation(174, context)?;
        let workspace = tempfile::tempdir()?;
        accept_prepared(prepared(root.path(), workspace.path(), context)?, context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let image =
            trust.pin_updater(Channel::Release, 2, context)?.ok_or("Healthy image absent")?;
        let directory = image.directory().to_owned();
        let generation = image.generation().clone();
        drop(image);
        let symbols = directory.join("app/sugarsubstitute_updater.pdb");
        let exclusive =
            std::fs::OpenOptions::new().read(true).write(true).share_mode(0).open(&symbols)?;
        assert!(trust.pin_updater(Channel::Release, 2, context)?.is_some());
        assert!(
            crate::setup_storage::bundle::Payload::open(
                &directory,
                generation.package.as_str(),
                Component::Updater,
                context
            )
            .is_err()
        );
        drop(exclusive);
        std::fs::remove_file(&symbols)?;
        assert!(trust.pin_updater(Channel::Release, 2, context)?.is_some());
        assert!(
            crate::setup_storage::bundle::Payload::open(
                &directory,
                generation.package.as_str(),
                Component::Updater,
                context
            )
            .is_err()
        );
        std::fs::write(&symbols, b"damaged symbols")?;
        assert!(trust.pin_updater(Channel::Release, 2, context)?.is_some());
        assert!(
            crate::setup_storage::bundle::Payload::open(
                &directory,
                generation.package.as_str(),
                Component::Updater,
                context
            )
            .is_err()
        );
        assert_eq!(std::fs::read(symbols)?, b"damaged symbols");
        Ok(())
    })
}

#[test]
fn accepted_engine_runtime_rejects_unknown_libraries_and_changed_inventory() -> TestResult {
    run(|context| {
        let root = worker::installation(175, context)?;
        let workspace = tempfile::tempdir()?;
        accept_prepared(prepared(root.path(), workspace.path(), context)?, context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let image =
            trust.pin_updater(Channel::Release, 2, context)?.ok_or("Healthy image absent")?;
        let directory = image.directory().to_owned();
        drop(image);
        let extra = directory.join("app/unexpected.dll");
        std::fs::write(&extra, b"unknown runtime")?;
        assert!(trust.pin_updater(Channel::Release, 2, context).is_err());
        assert_eq!(std::fs::read(&extra)?, b"unknown runtime");
        std::fs::remove_file(extra)?;
        let inventory = directory.join("setup-manifest.json");
        std::fs::write(&inventory, b"changed inventory")?;
        assert!(trust.pin_updater(Channel::Release, 2, context).is_err());
        assert_eq!(std::fs::read(inventory)?, b"changed inventory");
        Ok(())
    })
}

#[test]
fn missing_established_engine_selection_is_never_reinitialized_by_discovery() -> TestResult {
    run(|context| {
        let root = worker::installation(173, context)?;
        let workspace = tempfile::tempdir()?;
        accept_prepared(prepared(root.path(), workspace.path(), context)?, context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let journal = trust.root.path.join(SELECTION);
        std::fs::remove_file(&journal)?;
        assert!(trust.updater_request(Channel::Release, context).is_err());
        assert!(trust.pin_updater(Channel::Release, 2, context).is_err());
        assert!(!journal.exists());
        Ok(())
    })
}
