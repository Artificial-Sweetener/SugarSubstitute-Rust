// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Saved-data recovery is independent of old executables and never guesses conflicting settings.
use super::*;
use crate::setup_storage::{
    existing::fixture,
    test_worker::{self, TestResult},
};
use std::path::PathBuf;

/// Retain the same saved inputs in another completed generation without selecting it.
fn duplicate(root: &Directory, name: &str, context: &JobContext) -> io::Result<PathBuf> {
    let source =
        ExistingInstallation::read(root, context)?.ok_or_else(|| invalid("Missing fixture"))?;
    let destination = Directory::open(&root.path().join("releases").join(name), true)?;
    source.write_configuration(destination.path(), context)?;
    files::write_new(
        &destination.path().join("installation-ready.json"),
        &std::fs::read(source._directory.path().join("installation-ready.json"))?,
    )?;
    files::write_new(&destination.path().join("sugarsubstitute.exe"), b"inert fixture")?;
    Ok(destination.path().to_path_buf())
}

#[test]
fn equivalent_completed_generations_recover_exact_settings_and_ignore_partial_attempts()
-> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let original = fixture::installed(&root, context)?;
        let second = duplicate(&root, "second", context)?;
        let partial = Directory::open(&root.path().join("releases/partial"), true)?;
        files::write_new(&partial.path().join("startup.local.toml"), b"incomplete")?;
        let note = root.path().join("releases/user-note");
        files::write_new(&note, b"preserve")?;
        std::fs::remove_file(root.path().join("SugarSubstitute.lnk"))?;
        std::fs::remove_file(original.join("sugarsubstitute.exe"))?;
        let recovered = ExistingInstallation::read(&root, context)?.ok_or("Recovery absent")?;
        assert!(recovered.summary.launch_entry_missing);
        assert!(recovered.summary.executable_missing);
        assert_eq!(recovered._directory.path(), original);
        let output = Directory::open(&root.path().join("recovered"), true)?;
        recovered.write_configuration(output.path(), context)?;
        assert_eq!(
            std::fs::read(output.path().join("startup.local.toml"))?,
            std::fs::read(second.join("startup.local.toml"))?
        );
        assert_eq!(std::fs::read(&note)?, b"preserve");
        assert_eq!(std::fs::read(partial.path().join("startup.local.toml"))?, b"incomplete");
        assert!(!root.path().join("SugarSubstitute.lnk").exists());
        Ok(())
    })
}

#[test]
fn conflicting_completed_configurations_are_not_resolved_using_names_or_modification_times()
-> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        let original = fixture::installed(&root, context)?;
        let second = duplicate(&root, "newer-release", context)?;
        let changed = b"schema_version=1\n# Different user preferences\n";
        std::fs::write(second.join("startup.local.toml"), changed)?;
        std::fs::remove_file(root.path().join("SugarSubstitute.lnk"))?;
        assert!(inspect_installation(root.path(), context).is_err());
        assert_eq!(std::fs::read(second.join("startup.local.toml"))?, changed);
        assert!(original.join("startup.local.toml").exists());
        assert!(!root.path().join("SugarSubstitute.lnk").exists());
        Ok(())
    })
}

#[test]
fn malformed_completed_record_cannot_be_skipped_to_adopt_an_older_configuration() -> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        fixture::installed(&root, context)?;
        let second = duplicate(&root, "second", context)?;
        std::fs::write(second.join("installation-ready.json"), b"{corrupt")?;
        std::fs::remove_file(root.path().join("SugarSubstitute.lnk"))?;
        assert!(inspect_installation(root.path(), context).is_err());
        assert_eq!(std::fs::read(second.join("installation-ready.json"))?, b"{corrupt");
        Ok(())
    })
}

#[test]
fn interrupted_only_installation_is_preserved_without_becoming_fresh_setup() -> TestResult {
    test_worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        crate::setup_storage::ownership::establish(&root)?;
        let partial = Directory::open(&root.path().join("releases/partial"), true)?;
        files::write_new(&partial.path().join("startup.local.toml"), b"preserved partial input")?;
        assert!(inspect_installation(root.path(), context).is_err());
        assert_eq!(
            std::fs::read(partial.path().join("startup.local.toml"))?,
            b"preserved partial input"
        );
        Ok(())
    })
}

#[test]
fn recovery_inventory_cannot_ignore_entries_beyond_its_bound() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().to_owned();
    std::fs::create_dir(path.join("releases"))?;
    for index in 0..MAX_ENTRIES {
        std::fs::write(path.join("releases").join(format!("note-{index}")), b"keep")?;
    }
    // Fixture population and cleanup are outside the observer for the bounded inspection job.
    test_worker::run(move |context, _| {
        let root = Directory::open(&path, false)?;
        fixture::installed(&root, context)?;
        std::fs::remove_file(root.path().join("SugarSubstitute.lnk"))?;
        let failure = inspect_installation(root.path(), context)
            .err()
            .ok_or("Oversized inventory was accepted")?;
        assert_eq!(
            failure.get_ref().and_then(|cause| cause.downcast_ref::<RecoveryFailure>()),
            Some(&RecoveryFailure::InventoryBound)
        );
        assert_eq!(std::fs::read_dir(root.path().join("releases"))?.count(), MAX_ENTRIES + 1);
        Ok(())
    })
}
