// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise launch-selection custody independently of real OS application-run exclusion.
use super::*;
use crate::setup_storage::ReservedLaunch;

fn next_generation() -> Result<Generation, Box<dyn std::error::Error + Send + Sync>> {
    Ok(Generation {
        id: GenerationId::new("2".repeat(32))?,
        package: PackageDigest::new("c".repeat(64))?,
        release: sugarsubstitute_contracts::installation::SignedRelease::new(
            "1.0.0".to_owned(),
            1,
        )?,
    })
}

#[test]
fn launch_reservation_coexists_with_a_running_app_and_never_blocks_selection_writes() -> TestResult
{
    worker::run_reserved(TRUST_RESERVATION_BYTES + 96 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        let (generation, workspace) = accept_fixture(&directory, context)?;
        assert!(!workspace.exists());
        let store = Arc::new(SelectionStore::open(directory.path(), context)?);
        let before = store.load(context)?.ok_or("Selection absent")?;
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let InstallationLaunch::Application(running) = run.select_launch(None, context)? else {
            return Err("Running application absent".into());
        };
        let ReservedLaunch::Application(reserved) = store.reserve_launch(context)? else {
            return Err("Reservation incorrectly competed with the application".into());
        };
        assert_eq!(reserved.generation(), &generation);
        assert_eq!(reserved.executable(), running.executable());
        assert_eq!(reserved.directory(), running.directory());
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        let next = before.prepare(next_generation()?, PreparationIntent::Automatic)?;
        // The reservation is a value, never a held record handle, so writes proceed.
        store.compare_exchange(&before, &next, context)?;
        assert_eq!(store.load(context)?, Some(next));
        drop(reserved);
        drop(run);
        assert!(InstallationRun::acquire(store.clone(), context).is_err());
        drop(running);
        assert!(InstallationRun::acquire(store, context).is_ok());
        Ok(())
    })
}

#[test]
fn coordinator_can_acquire_its_run_and_publish_while_concurrent_launches_hold_reservations()
-> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 112 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        accept_fixture(&directory, context)?;
        let store = Arc::new(SelectionStore::open(directory.path(), context)?);
        let before = store.load(context)?.ok_or("Selection absent")?;
        let ReservedLaunch::Application(first) = store.reserve_launch(context)? else {
            return Err("First reservation absent".into());
        };
        let ReservedLaunch::Application(second) = store.reserve_launch(context)? else {
            return Err("Second reservation absent".into());
        };
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let running = run.adopt_reserved(first, context)?;
        let next = before.prepare(next_generation()?, PreparationIntent::Automatic)?;
        store.compare_exchange(&before, &next, context)?;
        drop(second);
        assert_eq!(running.generation(), before.accepted().ok_or("Accepted generation absent")?);
        Ok(())
    })
}

#[test]
fn failed_payload_verification_releases_selection_custody_without_creating_a_run_owner()
-> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        for leaf in ["application-package.json", "sugarsubstitute.exe"] {
            let temporary = tempfile::tempdir()?;
            let directory = Directory::open(temporary.path(), false)?;
            let (generation, _) = accept_fixture(&directory, context)?;
            let path = directory.path().join("releases").join(generation.id.as_str()).join(leaf);
            let mut edited = std::fs::read(&path)?;
            edited[0] ^= 1;
            std::fs::write(&path, &edited)?;
            let store = Arc::new(SelectionStore::open(directory.path(), context)?);
            let before = store.load(context)?.ok_or("Selection absent")?;
            assert!(store.reserve_launch(context).is_err());
            assert_eq!(store.load(context)?, Some(before.clone()));
            let next = before.prepare(next_generation()?, PreparationIntent::Automatic)?;
            store.compare_exchange(&before, &next, context)?;
            assert!(InstallationRun::acquire(store, context).is_ok());
            assert_eq!(std::fs::read(path)?, edited);
        }
        Ok(())
    })
}

#[test]
fn reservation_preserves_trials_and_empty_installations_as_explicit_nonlaunch_results() -> TestResult
{
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = crate::setup_storage::test_worker::installation(71, context)?;
        let store = Arc::new(SelectionStore::open(temporary.path(), context)?);
        assert!(matches!(store.reserve_launch(context)?, ReservedLaunch::NeedsInstallation));
        assert!(!temporary.path().join("state/selection/.installation-run-lock").exists());
        let before = store.load(context)?.ok_or("Selection absent")?;
        let generation = next_generation()?;
        let staging = before.prepare(generation.clone(), PreparationIntent::Automatic)?;
        store.compare_exchange(&before, &staging, context)?;
        let prepared = staging.prepared(&generation)?;
        store.compare_exchange(&staging, &prepared, context)?;
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let trial = run.begin_trial(&prepared, context)?;
        let expected = trial.attempt().clone();
        drop(trial);
        drop(run);
        let before = store.load(context)?;
        let ReservedLaunch::ReconcileTrial(actual) = store.reserve_launch(context)? else {
            return Err("Trial became an executable reservation".into());
        };
        assert_eq!(actual, expected);
        assert_eq!(store.load(context)?, before);
        assert!(InstallationRun::acquire(store, context).is_ok());
        assert!(!temporary.path().join("releases").exists());
        Ok(())
    })
}

#[test]
fn invalid_selection_is_preserved_and_does_not_leave_an_unreleaseable_read_pin() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        accept_fixture(&directory, context)?;
        let store = Arc::new(SelectionStore::open(directory.path(), context)?);
        let path = directory.path().join("state/selection/.substitute-selection.json");
        let before = std::fs::read(&path)?;
        std::fs::write(&path, b"invalid preserved selection")?;
        assert!(store.reserve_launch(context).is_err());
        assert_eq!(std::fs::read(&path)?, b"invalid preserved selection");
        std::fs::write(&path, before)?;
        assert!(matches!(store.reserve_launch(context)?, ReservedLaunch::Application(_)));
        Ok(())
    })
}

#[test]
fn reservation_cannot_be_adopted_by_a_foreign_native_installation_with_matching_ids() -> TestResult
{
    use sugarsubstitute_contracts::installation::InstallationRecord;
    worker::run_reserved(TRUST_RESERVATION_BYTES + 96 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        let (generation, _) = accept_fixture(&directory, context)?;
        let store = Arc::new(SelectionStore::open(directory.path(), context)?);
        let before = store.load(context)?.ok_or("Selection absent")?;
        let foreign_root = tempfile::tempdir()?;
        let foreign_directory = Directory::open(foreign_root.path(), false)?;
        fixture::installed(&foreign_directory, context)?;
        let initial = InstallationRecord::empty(before.installation().clone());
        let foreign = Arc::new(SelectionStore::initialize(foreign_root.path(), &initial, context)?);
        let staging = initial.prepare(generation.clone(), PreparationIntent::Automatic)?;
        foreign.compare_exchange(&initial, &staging, context)?;
        let prepared = staging.prepared(&generation)?;
        foreign.compare_exchange(&staging, &prepared, context)?;
        let mut seed = InstallationRun::acquire(foreign.clone(), context)?;
        let trial = seed.begin_trial(&prepared, context)?;
        seed.accept(&trial, context)?;
        drop(trial);
        drop(seed);
        let foreign_before = foreign.load(context)?;
        let mut foreign_run = InstallationRun::acquire(foreign.clone(), context)?;
        let ReservedLaunch::Application(reserved) = store.reserve_launch(context)? else {
            return Err("Reservation absent".into());
        };
        assert!(foreign_run.adopt_reserved(reserved, context).is_err());
        assert_eq!(foreign.load(context)?, foreign_before);
        assert_eq!(store.load(context)?, Some(before.clone()));
        let next = before.prepare(next_generation()?, PreparationIntent::Automatic)?;
        store.compare_exchange(&before, &next, context)?;
        Ok(())
    })
}

#[test]
fn repeated_adoption_is_rejected_without_leaking_selection_custody() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 96 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        accept_fixture(&directory, context)?;
        let store = Arc::new(SelectionStore::open(directory.path(), context)?);
        let before = store.load(context)?.ok_or("Selection absent")?;
        let ReservedLaunch::Application(reserved) = store.reserve_launch(context)? else {
            return Err("Reservation absent".into());
        };
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let running = run.adopt_reserved(reserved, context)?;
        let ReservedLaunch::Application(second) = store.reserve_launch(context)? else {
            return Err("Repeated launch reservation absent".into());
        };
        assert!(run.adopt_reserved(second, context).is_err());
        let next = before.prepare(next_generation()?, PreparationIntent::Automatic)?;
        store.compare_exchange(&before, &next, context)?;
        assert_eq!(running.generation(), before.accepted().ok_or("Accepted generation absent")?);
        Ok(())
    })
}

#[test]
fn expired_adoption_never_holds_or_rewrites_the_selection_record() -> TestResult {
    worker::run_reserved_expiring(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context, clock| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        accept_fixture(&directory, context)?;
        let store = Arc::new(SelectionStore::open(directory.path(), context)?);
        let path = directory.path().join("state/selection/.substitute-selection.json");
        let before = std::fs::read(&path)?;
        let ReservedLaunch::Application(reserved) = store.reserve_launch(context)? else {
            return Err("Reservation absent".into());
        };
        let mut run = InstallationRun::acquire(store, context)?;
        drop(std::fs::OpenOptions::new().write(true).open(&path)?);
        clock.expire();
        assert!(run.adopt_reserved(reserved, context).is_err());
        let _unblocked = std::fs::OpenOptions::new().write(true).open(&path)?;
        assert_eq!(std::fs::read(&path)?, before);
        Ok(())
    })
}
