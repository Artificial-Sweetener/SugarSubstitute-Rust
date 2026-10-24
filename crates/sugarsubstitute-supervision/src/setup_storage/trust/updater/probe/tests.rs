// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real signed generations and native custody exercise probe validation without executing fixtures.
use super::*;
use crate::setup_storage::{
    self as storage, UnstartedUpdaterTrial, UpdaterPackage,
    test_package::updater::{archive, evidence_version},
    test_worker::{self as worker, TestClock, TestResult},
};
use std::sync::Arc;
use sugarsubstitute_contracts::application_launch::updater::CURRENT_UPDATER_CONTRACT;
use sugarsubstitute_update::fixture::NOW;

const RESERVATION: u64 = storage::SIGNED_INSTALLER_RESERVATION_BYTES
    + storage::UPDATER_STAGING_RESERVATION_BYTES
    + storage::RESERVATION_BYTES
    + storage::TRUST_OPERATION_RESERVATION_BYTES;

/// Admit the original signed package, staged copy and one additional probe verification together.
fn run(body: impl FnOnce(&JobContext, &TestClock) -> TestResult + Send + 'static) -> TestResult {
    worker::run_reserved(RESERVATION, body)
}

/// Publish an inert but fully authenticated engine attempt with a chosen signed release sequence.
fn prepare(
    root: &Path,
    workspace: &Path,
    sequence: u32,
    context: &JobContext,
) -> Result<UnstartedUpdaterTrial, Box<dyn std::error::Error + Send + Sync>> {
    let (bytes, _) = archive(CURRENT_UPDATER_CONTRACT)?;
    let (trust, offer, download) =
        evidence_version(root, &bytes, Component::Updater, sequence, context)?;
    let mut trial =
        UpdaterPackage::import(root, trust, offer, download, workspace, || NOW, context)?
            .stage(|| NOW, context)?
            .reserve_trial(context)?;
    trial.publish(|| NOW, context)?;
    Ok(trial)
}

/// Use the actual compiled release rather than a fixture-specific successful engine version.
fn release() -> Result<u32, std::num::TryFromIntError> {
    u32::try_from(sugarsubstitute_update::ENGINE_RELEASE)
}

#[test]
fn exact_probe_is_read_only_and_independent_of_application_and_preparation_ownership() -> TestResult
{
    run(|context, _| {
        let root = worker::installation(130, context)?;
        let workspace = tempfile::tempdir()?;
        let mut trial = prepare(root.path(), workspace.path(), release()?, context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let executable =
            trust.updater_path(&trial.attempt().generation).join("app/sugarsubstitute-updater.exe");
        let selection = Arc::new(storage::SelectionStore::open(root.path(), context)?);
        let app = selection.load(context)?;
        let _run = storage::InstallationRun::acquire(Arc::clone(&selection), context)?;
        let _setup = storage::transaction::admission::Admission::open(root.path(), false)?;
        let before = [SELECTION, MARKER, "checkpoint", "releases"]
            .map(|name| std::fs::read(trust.root.path.join(name)))
            .into_iter()
            .collect::<io::Result<Vec<_>>>()?;
        trust.verify_updater_probe(&executable, trial.attempt(), context)?;
        for (name, bytes) in [SELECTION, MARKER, "checkpoint", "releases"].into_iter().zip(before) {
            assert_eq!(std::fs::read(trust.root.path.join(name))?, bytes);
        }
        assert_eq!(selection.load(context)?, app);
        assert!(trust.updater_selection(context)?.accepted().is_none());
        assert!(trust.claim_updater_preparation(context).is_err());
        trial.restore(context)?;
        assert!(trust.verify_updater_probe(&executable, trial.attempt(), context).is_err());
        Ok(())
    })
}

#[test]
fn foreign_attempts_and_same_named_executables_cannot_satisfy_probe_identity() -> TestResult {
    run(|context, _| {
        let root = worker::installation(131, context)?;
        let workspace = tempfile::tempdir()?;
        let trial = prepare(root.path(), workspace.path(), release()?, context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let executable =
            trust.updater_path(&trial.attempt().generation).join("app/sugarsubstitute-updater.exe");
        let before = trust.updater_selection(context)?;
        let mut stale = trial.attempt().clone();
        stale.revision += 1;
        assert!(trust.verify_updater_probe(&executable, &stale, context).is_err());
        stale = trial.attempt().clone();
        stale.installation =
            sugarsubstitute_contracts::installation::InstallationId::new(format!("{:032x}", 200))?;
        assert!(trust.verify_updater_probe(&executable, &stale, context).is_err());
        let foreign = workspace.path().join("sugarsubstitute-updater.exe");
        std::fs::copy(&executable, &foreign)?;
        for path in [&foreign, &executable.with_file_name("sugarsubstitute.exe")] {
            assert!(trust.verify_updater_probe(path, trial.attempt(), context).is_err());
        }
        assert_eq!(trust.updater_selection(context)?, before);
        Ok(())
    })
}

#[test]
fn a_newer_signed_receipt_cannot_make_an_older_compiled_engine_healthy() -> TestResult {
    run(|context, _| {
        let root = worker::installation(132, context)?;
        let workspace = tempfile::tempdir()?;
        let trial = prepare(
            root.path(),
            workspace.path(),
            release()?.checked_add(1).ok_or("release overflow")?,
            context,
        )?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let executable =
            trust.updater_path(&trial.attempt().generation).join("app/sugarsubstitute-updater.exe");
        let before = trust.updater_selection(context)?;
        assert!(trust.verify_updater_probe(&executable, trial.attempt(), context).is_err());
        assert_eq!(trust.updater_selection(context)?, before);
        Ok(())
    })
}

#[test]
fn probes_never_reinitialize_missing_engine_continuity_or_selection() -> TestResult {
    run(|context, _| {
        for missing in [MARKER, SELECTION] {
            let root = worker::installation(133, context)?;
            let workspace = tempfile::tempdir()?;
            let trial = prepare(root.path(), workspace.path(), release()?, context)?;
            let trust = TrustStore::open(root.path(), [92; 32], context)?;
            let executable = trust
                .updater_path(&trial.attempt().generation)
                .join("app/sugarsubstitute-updater.exe");
            std::fs::remove_file(trust.root.path.join(missing))?;
            assert!(trust.verify_updater_probe(&executable, trial.attempt(), context).is_err());
            assert!(!trust.root.path.join(missing).exists());
        }
        Ok(())
    })
}

#[test]
fn probe_rechecks_actual_payload_bytes_after_creator_custody_has_retired() -> TestResult {
    run(|context, _| {
        let root = worker::installation(134, context)?;
        let workspace = tempfile::tempdir()?;
        let trial = prepare(root.path(), workspace.path(), release()?, context)?;
        let attempt = trial.attempt().clone();
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let executable =
            trust.updater_path(&attempt.generation).join("app/sugarsubstitute-updater.exe");
        let before = trust.updater_selection(context)?;
        drop(trial);
        std::fs::write(&executable, b"damaged engine")?;
        assert!(trust.verify_updater_probe(&executable, &attempt, context).is_err());
        assert_eq!(std::fs::read(&executable)?, b"damaged engine");
        assert_eq!(trust.updater_selection(context)?, before);
        Ok(())
    })
}

#[test]
fn expired_probe_job_leaves_the_original_pending_attempt_intact() -> TestResult {
    worker::run_reserved_expiring(RESERVATION, |context, clock| {
        let root = worker::installation(135, context)?;
        let workspace = tempfile::tempdir()?;
        let trial = prepare(root.path(), workspace.path(), release()?, context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let executable =
            trust.updater_path(&trial.attempt().generation).join("app/sugarsubstitute-updater.exe");
        let before = std::fs::read(trust.root.path.join(SELECTION))?;
        clock.expire();
        assert!(trust.verify_updater_probe(&executable, trial.attempt(), context).is_err());
        assert_eq!(std::fs::read(trust.root.path.join(SELECTION))?, before);
        Ok(())
    })
}
