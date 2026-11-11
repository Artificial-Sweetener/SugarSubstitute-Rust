// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover saved first-install settings after a lost trial without reauthorizing its failed payload.
use super::*;
use crate::setup_storage::{CompletedInstallation, ReservedLaunch, reserve_installed_launch};

/// Publish a real trial and lose its native custody, exercising the ordinary quarantine owner.
fn quarantine(path: &Path, context: &JobContext) -> Result<CompletedInstallation, Failure> {
    let runtime = runtime(path)?;
    let completed = import(path, context)?
        .prepare_installation(|| NOW, context)?
        .complete(&runtime, || NOW, context)?
        .authorize_installation(|| NOW, context)?
        .publish(context)?;
    lose_trial(&completed, context)?;
    Ok(completed)
}

/// Inspection during a live trial is read-only; lost custody quarantines its execution choice.
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
    assert!(record.candidate().is_none());
    assert_eq!(record.rejected().len(), 1);
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
        assert!(InstalledCandidate::reopen(root, context)?.is_none());
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
        lose_trial(&second, context)?;
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
