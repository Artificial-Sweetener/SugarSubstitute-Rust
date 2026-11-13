// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real signed storage and file pins qualify the receiving engine without executing fixture bytes.
use super::*;
use crate::setup_storage::{
    self as storage, PreparedUpdater, UpdaterPackage,
    test_package::updater::{accept_prepared, archive, evidence_version},
    test_worker::{self as worker, TestResult},
};
use std::{path::PathBuf, sync::Arc};
use sugarsubstitute_contracts::{
    application_launch::updater::CURRENT_UPDATER_CONTRACT, update::Channel,
};
use sugarsubstitute_update::fixture::NOW;

const RESERVATION: u64 = storage::SIGNED_INSTALLER_RESERVATION_BYTES
    + storage::UPDATER_STAGING_RESERVATION_BYTES
    + storage::RESERVATION_BYTES
    + storage::TRUST_OPERATION_RESERVATION_BYTES;

/// Staging remains genuine; only the external probe's observed result is supplied by the fixture.
fn prepare(
    root: &Path,
    workspace: &Path,
    sequence: u32,
    context: &JobContext,
) -> Result<(PreparedUpdater, PathBuf), Box<dyn std::error::Error + Send + Sync>> {
    let (bytes, _) = archive(CURRENT_UPDATER_CONTRACT)?;
    let (trust, offer, download) =
        evidence_version(root, &bytes, Component::Updater, sequence, context)?;
    let prepared =
        UpdaterPackage::import(root, trust, offer, download, workspace, || NOW, context)?
            .stage(|| NOW, context)?;
    let trust = TrustStore::open(root, [92; 32], context)?;
    let executable =
        trust.updater_path(prepared.generation()).join("app/sugarsubstitute-updater.exe");
    Ok((prepared, executable))
}

fn release() -> Result<u32, std::num::TryFromIntError> {
    sugarsubstitute_update::ENGINE_RELEASE.try_into()
}

#[test]
fn separately_admitted_running_image_leaves_capacity_for_cached_startup() -> TestResult {
    use crate::update_preparation::{CachedConfiguration, Progress, Worker};
    use sugarsubstitute_contracts::execution::{JobOutcome, Moment};
    let mut pipeline = worker::pipeline::Pipeline::new()?;
    let (root, workspace, executable) = pipeline.run(RESERVATION, |context| {
        let root = worker::installation(188, &context)?;
        let workspace = tempfile::tempdir()?;
        let (prepared, executable) = prepare(root.path(), workspace.path(), release()?, &context)?;
        accept_prepared(prepared, &context)?;
        Ok((root, workspace, executable))
    })?;
    let installation = root.path().to_owned();
    let trust = pipeline.run(storage::TRUST_RESERVATION_BYTES, move |context| {
        Ok(TrustStore::open(&installation, [92; 32], &context)?)
    })?;
    let image = pipeline.run(
        storage::RESERVATION_BYTES + storage::TRUST_OPERATION_RESERVATION_BYTES,
        move |context| {
            let image = trust.retain_running_updater(&executable, &context)?;
            image.require_engine()?;
            Ok(image)
        },
    )?;
    let mut startup = Worker::resume_cached(CachedConfiguration {
        installation: root.path().to_owned(),
        workspace: workspace.path().to_owned(),
    })?;
    pipeline.admit_available(|client, scope, id| {
        startup.submit(client, scope, id, Moment::from_nanos(2_000_000_000), || NOW)
    })?;
    let observed = pipeline.observe(|| startup.poll())?;
    assert!(matches!(observed.outcome, JobOutcome::Success(())));
    assert_eq!(observed.progress, Progress::Restoring);
    assert!(std::fs::OpenOptions::new().write(true).open(image.executable()).is_err());
    startup.cancel();
    pipeline.admit_available(|client, scope, id| {
        startup.submit(client, scope, id, Moment::from_nanos(2_000_000_000), || NOW)
    })?;
    let retired = pipeline.observe(|| startup.poll())?;
    assert!(matches!(retired.outcome, JobOutcome::Success(())));
    assert_eq!(retired.progress, Progress::Closed);
    pipeline.run(0, move |_| {
        let executable = image.executable();
        drop(image);
        assert!(std::fs::OpenOptions::new().write(true).open(executable).is_ok());
        Ok(())
    })
}

#[test]
fn accepted_running_image_pins_bytes_without_application_or_preparation_exclusions() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let root = worker::installation(181, context)?;
        let workspace = tempfile::tempdir()?;
        let (prepared, executable) = prepare(root.path(), workspace.path(), release()?, context)?;
        accept_prepared(prepared, context)?;
        let mut trust = TrustStore::open(root.path(), [92; 32], context)?;
        let before = trust.updater_selection(context)?;
        let selection = Arc::new(storage::SelectionStore::open(root.path(), context)?);
        let application = selection.load(context)?;
        let _run = storage::InstallationRun::acquire(Arc::clone(&selection), context)?;
        let _setup = storage::transaction::admission::Admission::open(root.path(), false, context)?;
        trust.acquire_preparation(context)?;
        let _engine_preparation = trust.claim_updater_preparation(context)?;
        let image = trust.retain_running_updater(&executable, context)?;
        assert_eq!(image.generation(), before.accepted().ok_or("Accepted engine absent")?);
        assert_eq!(image.executable(), executable);
        assert!(std::fs::OpenOptions::new().write(true).open(&executable).is_err());
        assert_eq!(trust.updater_selection(context)?, before);
        assert_eq!(selection.load(context)?, application);
        drop(image);
        assert!(std::fs::OpenOptions::new().write(true).open(&executable).is_ok());
        Ok(())
    })
}

#[test]
fn accepted_receipt_cannot_upgrade_the_running_executables_compiled_release() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let root = worker::installation(182, context)?;
        let workspace = tempfile::tempdir()?;
        let next = release()?.checked_add(1).ok_or("Release overflow")?;
        let (prepared, executable) = prepare(root.path(), workspace.path(), next, context)?;
        accept_prepared(prepared, context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let before = trust.updater_selection(context)?;
        assert!(trust.retain_running_updater(&executable, context).is_err());
        assert_eq!(trust.updater_selection(context)?, before);
        Ok(())
    })
}

#[test]
fn foreign_copies_and_different_roles_cannot_act_as_the_running_engine() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let root = worker::installation(183, context)?;
        let workspace = tempfile::tempdir()?;
        let (prepared, executable) = prepare(root.path(), workspace.path(), release()?, context)?;
        accept_prepared(prepared, context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let foreign = workspace.path().join("sugarsubstitute-updater.exe");
        std::fs::copy(&executable, &foreign)?;
        for path in [&foreign, &executable.with_file_name("sugarsubstitute.exe")] {
            assert!(trust.retain_running_updater(path, context).is_err());
        }
        assert!(trust.retain_running_updater(&executable, context).is_ok());
        Ok(())
    })
}

#[test]
fn staged_and_trial_engines_cannot_satisfy_accepted_running_identity() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let root = worker::installation(184, context)?;
        let workspace = tempfile::tempdir()?;
        let (prepared, executable) = prepare(root.path(), workspace.path(), release()?, context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        assert!(trust.retain_running_updater(&executable, context).is_err());
        let mut trial = prepared.reserve_trial(context)?;
        trial.publish(|| NOW, context)?;
        let before = trust.updater_selection(context)?;
        assert!(trust.retain_running_updater(&executable, context).is_err());
        assert_eq!(trust.updater_selection(context)?, before);
        trial.restore(context)?;
        Ok(())
    })
}

#[test]
fn running_engine_cannot_reinitialize_missing_established_state() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        for missing in [MARKER, SELECTION] {
            let root = worker::installation(185, context)?;
            let workspace = tempfile::tempdir()?;
            let (prepared, executable) =
                prepare(root.path(), workspace.path(), release()?, context)?;
            accept_prepared(prepared, context)?;
            let trust = TrustStore::open(root.path(), [92; 32], context)?;
            let path = trust.root.path().join(missing);
            std::fs::remove_file(&path)?;
            assert!(trust.retain_running_updater(&executable, context).is_err());
            assert!(!path.exists());
        }
        Ok(())
    })
}

#[test]
fn subscription_changes_and_payload_damage_are_preserved_and_rejected() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let root = worker::installation(186, context)?;
        let workspace = tempfile::tempdir()?;
        let (prepared, executable) = prepare(root.path(), workspace.path(), release()?, context)?;
        accept_prepared(prepared, context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let before = trust.updater_selection(context)?;
        let subscription = trust.subscription(context)?;
        let canary = trust.select_subscription(subscription, Channel::Canary, context)?;
        assert!(trust.retain_running_updater(&executable, context).is_err());
        assert_eq!(trust.subscription(context)?, canary);
        trust.select_subscription(canary, Channel::Release, context)?;
        std::fs::write(&executable, b"damaged engine")?;
        assert!(trust.retain_running_updater(&executable, context).is_err());
        assert_eq!(std::fs::read(&executable)?, b"damaged engine");
        assert_eq!(trust.updater_selection(context)?, before);
        Ok(())
    })
}

#[test]
fn expired_running_image_verification_preserves_accepted_selection() -> TestResult {
    worker::run_reserved_expiring(RESERVATION, |context, clock| {
        let root = worker::installation(187, context)?;
        let workspace = tempfile::tempdir()?;
        let (prepared, executable) = prepare(root.path(), workspace.path(), release()?, context)?;
        accept_prepared(prepared, context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let before = std::fs::read(trust.root.path().join(SELECTION))?;
        clock.expire();
        assert!(trust.retain_running_updater(&executable, context).is_err());
        assert_eq!(std::fs::read(trust.root.path().join(SELECTION))?, before);
        Ok(())
    })
}
