// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise settled selection through real protected storage and continuously held run custody.
use super::*;
use crate::setup_storage::selection::StrikeClock;
use crate::setup_storage::{
    selection::{
        run::tests::{BAD, NO_VERDICT, prepare},
        tests::{TestResult, generation, initial, root},
    },
    test_worker as worker,
};
use sugarsubstitute_contracts::installation::PreparationIntent;
mod contention;

/// The injected time bad verdicts are dated with and withholding is checked at.
const AT: u64 = 1_800_000_456;

#[test]
fn a_bad_verdict_settles_once_and_preserves_later_preparation() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(
            SelectionStore::initialize(root.path(), &initial()?, context)?
                .with_strike_clock(StrikeClock::Fixed(AT)),
        );
        let prepared = prepare(&store, &initial()?, 1, context)?;
        let mut first_run = InstallationRun::acquire(store.clone(), context)?;
        let first_trial = first_run.begin_trial(&prepared, context)?;
        let healthy = first_run.accept(&first_trial, context)?;
        drop(first_trial);
        drop(first_run);
        let prepared = prepare(&store, &healthy, 2, context)?;
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let trial = run.begin_trial(&prepared, context)?;
        let rejected = run.finish_trial(&trial, BAD, context)?;
        assert_eq!(rejected.accepted(), healthy.accepted());
        assert_eq!(rejected.rollback(), Some(&generation(2)?));
        assert_eq!(rejected.strikes().len(), 1);
        assert_eq!(rejected.strikes()[0].at, AT, "the strike is dated by the store clock");
        assert!(rejected.candidate().is_none());
        assert_eq!(run.finish_trial(&trial, BAD, context)?, rejected);
        assert_eq!(run.finish_trial(&trial, NO_VERDICT, context)?, rejected);
        let next = prepare(&store, &rejected, 3, context)?;
        assert_eq!(run.finish_trial(&trial, BAD, context)?, next);
        assert_eq!(store.load(context)?, Some(next));
        Ok(())
    })
}

#[test]
fn no_verdict_returns_the_candidate_to_prepared_and_the_next_launch_accepts_it() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let prepared = prepare(&store, &initial()?, 1, context)?;
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let trial = run.begin_trial(&prepared, context)?;
        let released = run.finish_trial(&trial, NO_VERDICT, context)?;
        assert_eq!(released.candidate(), Some(&Candidate::Prepared { generation: generation(1)? }));
        assert!(released.strikes().is_empty());
        assert!(released.rollback().is_none());
        // The custody witness settling the same attempt later keeps the first verdict.
        assert_eq!(run.finish_trial(&trial, BAD, context)?, released);
        drop(trial);
        drop(run);
        let mut next_launch = InstallationRun::acquire(store.clone(), context)?;
        let retry = next_launch.begin_trial(&released, context)?;
        let accepted = next_launch.accept(&retry, context)?;
        assert_eq!(accepted.accepted(), Some(&generation(1)?));
        assert_eq!(store.load(context)?, Some(accepted));
        Ok(())
    })
}

#[test]
fn two_bad_trials_withhold_the_release_from_automatic_preparation_only() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(
            SelectionStore::initialize(root.path(), &initial()?, context)?
                .with_strike_clock(StrikeClock::Fixed(AT)),
        );
        let mut record = initial()?;
        for _ in 0..2 {
            let staging = record.prepare(generation(1)?, PreparationIntent::Repair, AT)?;
            store.compare_exchange(&record, &staging, context)?;
            let prepared = staging.prepared(&generation(1)?)?;
            store.compare_exchange(&staging, &prepared, context)?;
            let mut run = InstallationRun::acquire(store.clone(), context)?;
            let trial = run.begin_trial(&prepared, context)?;
            record = run.finish_trial(&trial, BAD, context)?;
        }
        let release = generation(1)?.release.sequence();
        assert!(record.strikes().iter().all(|strike| strike.at == AT));
        assert!(record.withheld_release(release, AT));
        assert_eq!(
            record.prepare(generation(1)?, PreparationIntent::Automatic, AT),
            Err(sugarsubstitute_contracts::installation::Rejection::Quarantined)
        );
        assert!(record.prepare(generation(1)?, PreparationIntent::Repair, AT).is_ok());
        Ok(())
    })
}

#[test]
fn lost_acceptance_result_is_preserved_with_subsequent_preparation() -> TestResult {
    let retained = Arc::new(std::sync::Mutex::new(None));
    let output = retained.clone();
    worker::run_expiring(move |context, clock| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let prepared = prepare(&store, &initial()?, 1, context)?;
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let trial = run.begin_trial(&prepared, context)?;
        run.accept(&trial, context)?;
        *output.lock().map_err(|_| "Retention poisoned")? = Some((root, run, trial));
        clock.expire();
        Ok(())
    })?;
    worker::run(move |context, _| {
        let retained = retained.lock().map_err(|_| "Retention poisoned")?;
        let (_root, run, trial) = retained.as_ref().ok_or("Original trial custody lost")?;
        let store = &run.custody.store;
        let accepted = store.load(context)?.ok_or("Accepted state missing")?;
        assert_eq!(run.finish_trial(trial, BAD, context)?, accepted);
        let next = prepare(store, &accepted, 2, context)?;
        assert_eq!(run.finish_trial(trial, NO_VERDICT, context)?, next);
        assert!(next.strikes().is_empty());
        assert_eq!(next.accepted(), Some(&generation(1)?));
        Ok(())
    })
}

#[test]
fn foreign_custody_and_changed_activation_cannot_finalize_the_original_trial() -> TestResult {
    worker::run(|context, _| {
        let a = root()?;
        let b = root()?;
        let store = Arc::new(SelectionStore::initialize(a.path(), &initial()?, context)?);
        let foreign = Arc::new(SelectionStore::initialize(b.path(), &initial()?, context)?);
        let prepared = prepare(&store, &initial()?, 1, context)?;
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let trial = run.begin_trial(&prepared, context)?;
        let other = InstallationRun::acquire(foreign, context)?;
        assert!(matches!(
            other.finish_trial(&trial, BAD, context),
            Err(SelectionFailure::RunOwnership)
        ));
        let accepted = run.accept(&trial, context)?;
        let prepared = prepare(&store, &accepted, 2, context)?;
        // A hostile record cannot turn original custody into another attempt's authority.
        let changed = prepared.begin_trial()?;
        std::fs::write(store.root.path().join(super::super::super::RECORD), changed.encode()?)?;
        for verdict in [BAD, NO_VERDICT] {
            assert!(matches!(
                run.finish_trial(&trial, verdict, context),
                Err(SelectionFailure::Changed)
            ));
        }
        assert_eq!(store.load(context)?, Some(changed));
        Ok(())
    })
}
