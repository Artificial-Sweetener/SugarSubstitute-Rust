// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise abandoned-selection recovery through real exclusion and atomic protected storage.
use super::*;
use crate::setup_storage::selection::StrikeClock;
use crate::setup_storage::{
    selection::{
        run::tests::prepare,
        tests::{TestResult, generation, initial, root},
    },
    test_worker as worker,
};
use sugarsubstitute_contracts::installation::{UNWITNESSED_LIMIT, Unwitnessed};

/// The injected time an abandoned trial that counts as a strike is dated with.
const AT: u64 = 1_800_000_123;

/// Begin a trial of `prepared`, then lose every owner without settling it, as a power cut does.
fn abandon(
    store: &Arc<SelectionStore>,
    prepared: &InstallationRecord,
    context: &JobContext,
) -> TestResult {
    let mut run = InstallationRun::acquire(store.clone(), context)?;
    let trial = run.begin_trial(prepared, context)?;
    drop(trial);
    drop(run);
    Ok(())
}

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
        assert_eq!(
            recovered.candidate(),
            Some(&Candidate::Prepared { generation: generation(1)? })
        );
        assert!(recovered.strikes().is_empty());
        assert_eq!(store.load_recovered(context)?, recovered);
        Ok(())
    })
}

#[test]
fn an_abandoned_trial_activates_again_until_the_third_in_a_row_counts_as_a_strike() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(
            SelectionStore::initialize(root.path(), &initial()?, context)?
                .with_strike_clock(StrikeClock::Fixed(AT)),
        );
        let mut prepared = prepare(&store, &initial()?, 1, context)?;
        for count in 1..UNWITNESSED_LIMIT {
            abandon(&store, &prepared, context)?;
            let mut launch = InstallationRun::acquire(store.clone(), context)?;
            assert!(matches!(
                launch.select_launch(None, context)?,
                InstallationLaunch::NeedsInstallation
            ));
            prepared = store.load(context)?.ok_or("Recovered state absent")?;
            assert_eq!(
                prepared.candidate(),
                Some(&Candidate::Prepared { generation: generation(1)? })
            );
            assert_eq!(
                prepared.unwitnessed(),
                Some(&Unwitnessed { package: generation(1)?.package, count })
            );
        }
        abandon(&store, &prepared, context)?;
        let struck = store.load_recovered(context)?;
        assert!(struck.candidate().is_none());
        assert_eq!(struck.rollback(), Some(&generation(1)?));
        assert_eq!(struck.strikes().len(), 1);
        assert_eq!(struck.strikes()[0].at, AT, "recovery dates the strike with the store clock");
        assert!(struck.unwitnessed().is_none());
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
        assert!(pending.strikes().is_empty());
        Ok(())
    })
}

#[test]
fn expired_recovery_preserves_original_selection_bytes() -> TestResult {
    worker::run_expiring(|context, clock| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let prepared = prepare(&store, &initial()?, 1, context)?;
        abandon(&store, &prepared, context)?;
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
