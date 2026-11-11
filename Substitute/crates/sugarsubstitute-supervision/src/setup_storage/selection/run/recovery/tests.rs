// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise abandoned-selection recovery through real exclusion and atomic protected storage.
use super::*;
use crate::setup_storage::{
    selection::{
        run::tests::prepare,
        tests::{TestResult, generation, initial, root},
    },
    test_worker as worker,
};
use sugarsubstitute_contracts::installation::PreparationIntent;

#[test]
fn admitted_run_cannot_recover_its_own_trial() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let prepared = prepare(&store, &initial()?, 1, context)?;
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let trial = run.begin_trial(&prepared, context)?;
        let before = store.load(context)?;
        assert!(matches!(
            run.recover_abandoned_trial(context),
            Err(SelectionFailure::RunOwnership)
        ));
        assert_eq!(store.load(context)?, before);
        drop(trial);
        drop(run);
        let recovered = store.load_recovered(context)?;
        assert!(recovered.accepted().is_none());
        assert!(recovered.candidate().is_none());
        assert_eq!(recovered.rejected(), &[generation(1)?.package]);
        assert_eq!(store.load_recovered(context)?, recovered);
        Ok(())
    })
}

#[test]
fn abandoned_first_install_allows_explicit_repair_without_inventing_health() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let prepared = prepare(&store, &initial()?, 1, context)?;
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let trial = run.begin_trial(&prepared, context)?;
        drop(trial);
        drop(run);
        let mut recovered_run = InstallationRun::acquire(store.clone(), context)?;
        assert!(matches!(
            recovered_run.select_launch(None, context)?,
            InstallationLaunch::NeedsInstallation
        ));
        let recovered = store.load(context)?.ok_or("Recovered state absent")?;
        assert!(recovered.prepare(generation(1)?, PreparationIntent::Automatic).is_err());
        let retry = recovered.prepare(generation(1)?, PreparationIntent::Repair)?;
        store.compare_exchange(&recovered, &retry, context)?;
        assert_eq!(recovered_run.recover_abandoned_trial(context)?, retry);
        assert!(retry.accepted().is_none());
        Ok(())
    })
}

#[test]
fn recovery_preserves_committed_acceptance_and_independent_preparation() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let prepared = prepare(&store, &initial()?, 1, context)?;
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let trial = run.begin_trial(&prepared, context)?;
        let accepted = run.accept(&trial, context)?;
        drop(trial);
        drop(run);
        let run = InstallationRun::acquire(store.clone(), context)?;
        assert_eq!(run.recover_abandoned_trial(context)?, accepted);
        let pending = prepare(&store, &accepted, 2, context)?;
        assert_eq!(run.recover_abandoned_trial(context)?, pending);
        assert_eq!(store.load_recovered(context)?, pending);
        assert_eq!(pending.accepted(), accepted.accepted());
        assert!(pending.rejected().is_empty());
        Ok(())
    })
}

#[test]
fn expired_recovery_preserves_original_selection_bytes() -> TestResult {
    worker::run_expiring(|context, clock| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let prepared = prepare(&store, &initial()?, 1, context)?;
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let trial = run.begin_trial(&prepared, context)?;
        drop(trial);
        drop(run);
        let run = InstallationRun::acquire(store.clone(), context)?;
        let path = store.root.path().join(super::super::super::RECORD);
        let before = std::fs::read(&path)?;
        clock.expire();
        assert!(run.recover_abandoned_trial(context).is_err());
        assert_eq!(std::fs::read(path)?, before);
        Ok(())
    })
}

#[test]
fn recovery_preserves_corrupt_selection_bytes() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let run = InstallationRun::acquire(store.clone(), context)?;
        let path = store.root.path().join(super::super::super::RECORD);
        std::fs::write(&path, b"invalid selection")?;
        assert!(run.recover_abandoned_trial(context).is_err());
        assert_eq!(std::fs::read(path)?, b"invalid selection");
        Ok(())
    })
}
