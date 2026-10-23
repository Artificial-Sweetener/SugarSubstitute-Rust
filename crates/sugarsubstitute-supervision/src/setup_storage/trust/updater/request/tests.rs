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
        let executable = image.executable();
        drop(image);
        std::fs::write(&executable, b"damaged engine")?;
        assert!(trust.pin_updater(Channel::Release, 2, context).is_err());
        assert_eq!(std::fs::read(executable)?, b"damaged engine");
        assert_eq!(trust.updater_request(Channel::Release, context)?.sequence_floor, 2);
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
        let journal = trust.root.path().join(SELECTION);
        std::fs::remove_file(&journal)?;
        assert!(trust.updater_request(Channel::Release, context).is_err());
        assert!(trust.pin_updater(Channel::Release, 2, context).is_err());
        assert!(!journal.exists());
        Ok(())
    })
}
