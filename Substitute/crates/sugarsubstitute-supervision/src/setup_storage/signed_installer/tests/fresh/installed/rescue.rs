// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover saved first-install settings after a lost trial without reauthorizing its failed payload.
use super::*;
use crate::setup_storage::{CompletedInstallation, ReservedLaunch, reserve_installed_launch};

/// The fixed UTC Unix time the failed generation is prepared for repair at.
const REPAIRED_AT: u64 = 1_800_000_000;

/// Publish a real first installation whose own start fails until the installation withholds
/// the package: a lost trial changes nothing, and two bad verdicts for the same generation
/// make the two strikes.
fn quarantine(path: &Path, context: &JobContext) -> Result<CompletedInstallation, Failure> {
    use sugarsubstitute_contracts::installation::{BadTrial, TrialVerdict};
    let runtime = runtime(path)?;
    let completed = import(path, context)?
        .prepare_installation(|| NOW, context)?
        .complete(&runtime, || NOW, context)?
        .authorize_installation(|| NOW, context)?
        .publish(context)?;
    lose_trial(&completed, context)?;
    let store = Arc::new(SelectionStore::open(completed.root(), context)?);
    let first = store.load(context)?.ok_or("State absent")?;
    let mut run = InstallationRun::acquire(store.clone(), context)?;
    let trial = run.begin_trial(&first, context)?;
    let struck = run.finish_trial(&trial, TrialVerdict::Bad(BadTrial::Crashed), context)?;
    drop(trial);
    drop(run);
    let generation = struck.rollback().ok_or("Strike absent")?.clone();
    // Repair preparation ignores strikes, so the time it is given decides nothing.
    let staging = struck.prepare(generation.clone(), PreparationIntent::Repair, REPAIRED_AT)?;
    store.compare_exchange(&struck, &staging, context)?;
    let prepared = staging.prepared(&generation)?;
    store.compare_exchange(&staging, &prepared, context)?;
    let mut run = InstallationRun::acquire(store.clone(), context)?;
    let trial = run.begin_trial(&prepared, context)?;
    let record = run.finish_trial(&trial, TrialVerdict::Bad(BadTrial::Crashed), context)?;
    assert!(record.candidate().is_none());
    // The rescue below reads withholding on the real clock, so the strikes keep their real dates;
    // at the latest of them the package is withheld.
    let struck_at = record
        .strikes()
        .iter()
        .filter(|strike| strike.package == generation.package)
        .map(|strike| strike.at)
        .max()
        .ok_or("Strike absent")?;
    assert!(record.withheld(&generation.package, struck_at));
    Ok(completed)
}

/// Inspection during a live trial is read-only; lost custody leaves its trial abandoned, which
/// the next launch resolves without starting the unaccepted candidate.
fn lose_trial(completed: &CompletedInstallation, context: &JobContext) -> TestResult {
    let installed =
        InstalledCandidate::reopen(completed.root(), context)?.ok_or("Choice absent")?;
    let mut recovery = None;
    let transfer = installed.transfer(context, &mut recovery)?;
    assert!(matches!(
        reserve_installed_launch(completed.root(), context)?,
        ReservedLaunch::Application(_)
    ));
    let store = SelectionStore::open(completed.root(), context)?;
    let live = store.load(context)?;
    assert!(inspect_installation(completed.root(), context)?.is_some());
    assert_eq!(store.load(context)?, live);
    drop(transfer);
    drop(recovery);
    drop(installed);
    assert!(matches!(
        reserve_installed_launch(completed.root(), context)?,
        ReservedLaunch::NeedsInstallation
    ));
    let record =
        SelectionStore::open(completed.root(), context)?.load(context)?.ok_or("State absent")?;
    assert!(record.accepted().is_none());
    assert!(!matches!(
        record.candidate(),
        Some(sugarsubstitute_contracts::installation::Candidate::Trial { .. })
    ));
    Ok(())
}

#[test]
fn quarantined_first_installation_repairs_explicitly_without_resetting_saved_inputs() -> TestResult
{
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path();
        let completed = quarantine(path, context)?;
        let root = completed.root();
        let directory = completed.directory();
        let startup = std::fs::read(directory.join("startup.local.toml"))?;
        let original_python = std::fs::read(path.join("python.exe"))?;
        let authored = root.join("user/desktop-data/user-workflow.json");
        std::fs::write(&authored, b"authored content")?;
        std::fs::remove_file(directory.join("sugarsubstitute.exe"))?;
        let store = SelectionStore::open(root, context)?;
        let quarantined = store.load(context)?.ok_or("State absent")?;
        let summary = inspect_installation(root, context)?.ok_or("Repair settings absent")?;
        assert!(summary.selection == choices(path));
        assert!(summary.executable_missing);
        assert!(!summary.launch_entry_missing);
        assert!(InstalledCandidate::reopen(root, context)?.is_none());
        assert_eq!(store.load(context)?.as_ref(), Some(&quarantined));
        let session = InstallationSession::maintain(root, context)?;
        assert!(
            import_session(path, session, PreparationIntent::Automatic, context)?
                .prepare_installation(|| NOW, context)
                .is_err()
        );
        assert_eq!(store.load(context)?.as_ref(), Some(&quarantined));
        let session = InstallationSession::maintain(root, context)?;
        let prepared = import_session(path, session, PreparationIntent::Repair, context)?
            .prepare_installation(|| NOW, context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Saved runtime absent")?.clone();
        let repaired = prepared
            .complete(&runtime, || NOW, context)?
            .authorize_installation(|| NOW, context)?
            .publish(context)?;
        assert_ne!(repaired.directory(), directory);
        assert_eq!(std::fs::read(repaired.directory().join("startup.local.toml"))?, startup);
        assert_eq!(std::fs::read(directory.join("startup.local.toml"))?, startup);
        assert_eq!(std::fs::read(runtime.python)?, original_python);
        assert_eq!(std::fs::read(authored)?, b"authored content");
        assert!(repaired.directory().join("sugarsubstitute.exe").is_file());
        let installed = InstalledCandidate::reopen(root, context)?.ok_or("Repair choice absent")?;
        let mut recovery = None;
        let transfer = installed.transfer(context, &mut recovery)?;
        assert_eq!(transfer.directory(), repaired.directory());
        let record = store.load(context)?.ok_or("State absent")?;
        assert!(record.accepted().is_none());
        assert!(matches!(record.candidate(), Some(Candidate::Trial { .. })));
        Ok(())
    })
}

#[test]
fn stable_entry_recovery_ignores_unknown_completed_folders_without_selecting_a_payload()
-> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let completed = quarantine(temporary.path(), context)?;
        let unknown = completed.root().join("releases/user-notes");
        std::fs::create_dir(&unknown)?;
        std::fs::write(unknown.join("installation-ready.json"), b"unrelated user file")?;
        let selection = completed.root().join("state/selection/.substitute-selection.json");
        let before = std::fs::read(&selection)?;
        let summary = inspect_installation(completed.root(), context)?.ok_or("Settings absent")?;
        assert!(summary.selection == choices(temporary.path()));
        assert!(!summary.launch_entry_missing);
        assert_eq!(std::fs::read(selection)?, before);
        assert_eq!(std::fs::read(unknown.join("installation-ready.json"))?, b"unrelated user file");
        Ok(())
    })
}

#[test]
fn quarantined_configuration_recovery_preserves_unknown_entries_and_conflicting_package_binding()
-> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let completed = quarantine(temporary.path(), context)?;
        let root = completed.root();
        let entry = root.join("SugarSubstitute.exe");
        let original = std::fs::read(&entry)?;
        std::fs::remove_file(&entry)?;
        std::fs::write(&entry, b"unowned executable")?;
        assert!(inspect_installation(root, context).is_err());
        assert_eq!(std::fs::read(&entry)?, b"unowned executable");
        std::fs::remove_file(&entry)?;
        crate::durable_storage::protection::with(context, |protection| {
            use std::io::Write as _;
            protection.file(&entry, context)?.write_all(&original)
        })?;
        let ready = completed.directory().join("installation-ready.json");
        let mut record: serde_json::Value = serde_json::from_slice(&std::fs::read(&ready)?)?;
        record["bundle_sha256"] = serde_json::json!("f".repeat(64));
        let changed = serde_json::to_vec(&record)?;
        std::fs::write(&ready, &changed)?;
        assert!(inspect_installation(root, context).is_err());
        assert_eq!(std::fs::read(ready)?, changed);
        Ok(())
    })
}

#[test]
fn missing_owned_shell_entries_do_not_hide_quarantined_first_installation_settings() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let completed = quarantine(temporary.path(), context)?;
        let root = completed.root();
        std::fs::remove_file(root.join("SugarSubstitute.exe"))?;
        let summary = inspect_installation(root, context)?.ok_or("Settings absent")?;
        assert!(summary.selection == choices(temporary.path()));
        assert!(summary.launch_entry_missing);
        // The installed release is still startable; looking for it creates no shell entry.
        assert!(InstalledCandidate::reopen(root, context)?.is_some());
        assert!(!root.join("SugarSubstitute.exe").exists());
        Ok(())
    })
}

#[test]
fn stable_launcher_source_does_not_choose_between_conflicting_unaccepted_settings() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let first = quarantine(temporary.path(), context)?;
        let root = first.root();
        let session = InstallationSession::maintain(root, context)?;
        let prepared =
            import_session(temporary.path(), session, PreparationIntent::Repair, context)?
                .prepare_installation(|| NOW, context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Saved runtime absent")?.clone();
        let second = prepared
            .complete(&runtime, || NOW, context)?
            .authorize_installation(|| NOW, context)?
            .publish(context)?;
        // The second release fails its own start as well, so neither is a candidate any more.
        {
            use sugarsubstitute_contracts::installation::{BadTrial, TrialVerdict};
            let store = Arc::new(SelectionStore::open(second.root(), context)?);
            let prepared = store.load(context)?.ok_or("State absent")?;
            let mut run = InstallationRun::acquire(store, context)?;
            let trial = run.begin_trial(&prepared, context)?;
            run.finish_trial(&trial, TrialVerdict::Bad(BadTrial::Crashed), context)?;
        }
        let configuration = first.directory().join("startup.local.toml");
        let changed = b"schema_version = 1\n# Preserved user alternative\n";
        std::fs::write(&configuration, changed)?;
        let link = root.join("SugarSubstitute.exe");
        let original = std::fs::read(&link)?;
        let failure = match inspect_installation(root, context) {
            Err(cause) => cause,
            Ok(_) => return Err("Conflicting settings were accepted".into()),
        };
        let selection = failure
            .get_ref()
            .and_then(|cause| cause.downcast_ref::<SelectionFailure>())
            .ok_or("Selection failure absent")?;
        let SelectionFailure::Storage(storage) = selection else {
            return Err("Expected saved-configuration failure".into());
        };
        assert_eq!(
            storage
                .get_ref()
                .and_then(|cause| cause.downcast_ref::<crate::setup_storage::RecoveryFailure>()),
            Some(&crate::setup_storage::RecoveryFailure::ConflictingSettings)
        );
        assert_eq!(std::fs::read(configuration)?, changed);
        assert_eq!(std::fs::read(link)?, original);
        assert!(InstalledCandidate::reopen(root, context).is_err());
        Ok(())
    })
}

#[test]
fn a_first_installation_whose_trial_was_killed_is_offered_again_by_the_very_next_reopen()
-> TestResult {
    use sugarsubstitute_contracts::installation::UNWITNESSED_LIMIT;
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path();
        let runtime = runtime(path)?;
        let completed = import(path, context)?
            .prepare_installation(|| NOW, context)?
            .complete(&runtime, || NOW, context)?
            .authorize_installation(|| NOW, context)?
            .publish(context)?;
        // More kills than the abandoned-trial limit: nothing else can run, so none may withhold it.
        for _ in 0..UNWITNESSED_LIMIT + 2 {
            let installed =
                InstalledCandidate::reopen(completed.root(), context)?.ok_or("Choice absent")?;
            let mut recovery = None;
            let transfer = installed.transfer(context, &mut recovery)?;
            // Every owner is lost with the trial still on record, as a kill leaves it.
            drop(transfer);
            drop(recovery);
            drop(installed);
            let record = SelectionStore::open(completed.root(), context)?
                .load(context)?
                .ok_or("State absent")?;
            assert!(matches!(record.candidate(), Some(Candidate::Trial { .. })));
        }
        // The launcher's first question after the kill is this one; it must find the candidate.
        let installed = InstalledCandidate::reopen(completed.root(), context)?
            .ok_or("A killed first start left nothing to launch")?;
        assert_eq!(installed.directory(), completed.directory());
        let record = SelectionStore::open(completed.root(), context)?
            .load(context)?
            .ok_or("State absent")?;
        assert!(record.strikes().is_empty());
        Ok(())
    })
}

#[test]
fn a_first_installation_struck_by_its_own_failed_starts_is_startable_again_without_setup()
-> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let temporary = tempfile::tempdir()?;
        let completed = quarantine(temporary.path(), context)?;
        let root = completed.root();
        let store = SelectionStore::open(root, context)?;
        let struck = store.load(context)?.ok_or("State absent")?;
        assert!(struck.accepted().is_none() && struck.candidate().is_none());
        // Nothing is accepted and nothing is prepared, yet the installed release is on disk with
        // the choice setup recorded for it: the launcher starts it rather than refusing.
        let installed = InstalledCandidate::reopen(root, context)?
            .ok_or("A struck first installation had nothing to launch")?;
        let record = store.load(context)?.ok_or("State absent")?;
        assert!(matches!(record.candidate(), Some(Candidate::Prepared { .. })));
        assert_eq!(record.strikes(), struck.strikes(), "its history is kept");
        let mut recovery = None;
        let transfer = installed.transfer(context, &mut recovery)?;
        assert!(transfer.directory().join("sugarsubstitute.exe").is_file());
        Ok(())
    })
}
