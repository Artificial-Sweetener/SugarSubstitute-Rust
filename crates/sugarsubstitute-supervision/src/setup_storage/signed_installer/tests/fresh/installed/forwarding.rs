// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A duplicate trial invocation can forward without acquiring run authority or blocking health.
use super::*;
use crate::setup_storage::{
    CompletedInstallation, InstallationRun, OwnedTrial, ReservedApplication, ReservedLaunch,
    SelectionFailure, reserve_installed_launch,
};

/// Real protected installation with the original run and its sole unaccepted attempt retained.
type Running = (CompletedInstallation, Arc<SelectionStore>, InstallationRun, OwnedTrial);

/// Publish a signed first installation and begin its trial at the actual storage boundary.
fn running(path: &Path, context: &JobContext) -> Result<Running, Failure> {
    let runtime = runtime(path)?;
    let completed = import(path, context)?
        .prepare_installation(|| NOW, context)?
        .complete(&runtime, || NOW, context)?
        .authorize_installation(|| NOW, context)?
        .publish(context)?;
    let store = Arc::new(SelectionStore::open(completed.root(), context)?);
    let prepared = store.load(context)?.ok_or("Prepared selection absent")?;
    let mut run = InstallationRun::acquire(store.clone(), context)?;
    let trial = run.begin_trial(&prepared, context)?;
    Ok((completed, store, run, trial))
}

/// Require the public installed-launch entry to provide the verified forwarding reservation.
fn reserve(root: &Path, context: &JobContext) -> Result<ReservedApplication, Failure> {
    match reserve_installed_launch(root, context)? {
        ReservedLaunch::Application(application) => Ok(application),
        _ => Err("Live trial did not provide a forwarding reservation".into()),
    }
}

#[test]
fn duplicate_trial_reference_permits_health_and_never_retains_run_ownership() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let (completed, store, run, trial) = running(temporary.path(), context)?;
        let before = store.load(context)?;
        let reserved = reserve(completed.root(), context)?;
        assert_eq!(reserved.generation(), &trial.attempt().generation);
        assert_eq!(store.load(context)?, before);
        assert!(matches!(
            InstallationRun::acquire(store.clone(), context),
            Err(SelectionFailure::RunBusy(_))
        ));
        let inherited = reserved.prepare_inheritance(context)?;
        let accepted = run.accept(&trial, context)?;
        assert_eq!(accepted.accepted(), Some(&trial.attempt().generation));
        assert!(accepted.strikes().is_empty());
        drop(trial);
        drop(run);
        assert!(InstallationRun::acquire(store, context).is_ok());
        drop(inherited);
        for entry in std::fs::read_dir(completed.root().join("state/selection"))? {
            assert!(!entry?.file_name().to_string_lossy().starts_with(".selection-forward"));
        }
        Ok(())
    })
}

#[test]
fn duplicate_trial_reference_cannot_promote_itself_after_the_original_owner_exits() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let (completed, store, run, trial) = running(temporary.path(), context)?;
        let reserved = reserve(completed.root(), context)?;
        let before = store.load(context)?;
        drop(trial);
        drop(run);
        let mut elected = InstallationRun::acquire(store.clone(), context)?;
        assert!(elected.adopt_reserved(reserved, context).is_err());
        assert_eq!(store.load(context)?, before);
        drop(elected);
        assert!(matches!(
            reserve_installed_launch(completed.root(), context)?,
            ReservedLaunch::NeedsInstallation
        ));
        let after = store.load(context)?.ok_or("Recovered selection absent")?;
        assert!(after.accepted().is_none());
        assert!(matches!(
            after.candidate(),
            Some(sugarsubstitute_contracts::installation::Candidate::Prepared { .. })
        ));
        assert!(after.strikes().is_empty());
        assert_eq!(after.unwitnessed().map(|abandoned| abandoned.count), Some(1));
        Ok(())
    })
}

#[test]
fn busy_run_does_not_authorize_modified_trial_payloads() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let (completed, store, _run, _trial) = running(temporary.path(), context)?;
        let executable = completed.directory().join("sugarsubstitute.exe");
        std::fs::write(&executable, b"altered payload")?;
        let before = store.load(context)?;
        assert!(reserve_installed_launch(completed.root(), context).is_err());
        assert_eq!(store.load(context)?, before);
        assert_eq!(std::fs::read(executable)?, b"altered payload");
        Ok(())
    })
}

#[test]
fn malformed_run_rendezvous_is_not_classified_as_an_existing_trial_owner() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let (completed, store, run, trial) = running(temporary.path(), context)?;
        drop(trial);
        drop(run);
        let path = completed.root().join("state/selection/.installation-run-lock");
        std::fs::write(&path, b"unknown contents")?;
        let before = store.load(context)?;
        assert!(matches!(
            reserve_installed_launch(completed.root(), context),
            Err(SelectionFailure::Storage(_))
        ));
        assert_eq!(store.load(context)?, before);
        assert_eq!(std::fs::read(path)?, b"unknown contents");
        Ok(())
    })
}
