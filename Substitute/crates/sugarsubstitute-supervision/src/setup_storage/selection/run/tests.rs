// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real OS exclusion, independent preparation and attempt-bound publication.
use super::*;
use crate::setup_storage::{
    selection::{
        StrikeClock,
        tests::{PREPARED_AT, TestResult, generation, initial, root},
    },
    test_worker as worker,
};
use std::fs::File;
use sugarsubstitute_contracts::installation::{
    BadTrial, PreparationIntent, TrialVerdict, Undecided,
};

/// A bad verdict: the new host itself crashed.
pub(super) const BAD: TrialVerdict = TrialVerdict::Bad(BadTrial::Crashed);
/// No verdict: the app was asked to close before readiness.
pub(super) const NO_VERDICT: TrialVerdict = TrialVerdict::NoVerdict(Undecided::CloseRequested);

pub(super) fn prepare(
    store: &SelectionStore,
    expected: &InstallationRecord,
    number: u64,
    context: &JobContext,
) -> Result<InstallationRecord, SelectionFailure> {
    let generation = generation(number)?;
    let staging =
        expected.prepare(generation.clone(), PreparationIntent::Automatic, PREPARED_AT)?;
    store.compare_exchange(expected, &staging, context)?;
    let prepared = staging.prepared(&generation)?;
    store.compare_exchange(&staging, &prepared, context)?;
    Ok(prepared)
}

#[test]
fn unstarted_recovery_rebinds_only_the_original_native_installation_and_exact_snapshot()
-> TestResult {
    worker::run(|context, _| {
        let a = root()?;
        let b = root()?;
        let store = Arc::new(SelectionStore::initialize(a.path(), &initial()?, context)?);
        let foreign = SelectionStore::initialize(b.path(), &initial()?, context)?;
        let prepared = prepare(&store, &initial()?, 1, context)?;
        let mut other = prepare(&foreign, &initial()?, 1, context)?;
        assert_eq!(prepared, other);
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let recovery = run.reserve_unstarted(&prepared, context)?;
        let _trial = run.begin_trial(&prepared, context)?;
        let before = store.load(context)?;
        assert!(matches!(
            recovery.restore(&foreign._installation, &mut other, context),
            Err(SelectionFailure::RunOwnership)
        ));
        assert_eq!(other, prepared);
        let mut stale = initial()?;
        assert!(matches!(
            recovery.restore(&store._installation, &mut stale, context),
            Err(SelectionFailure::Changed)
        ));
        assert_eq!(store.load(context)?, before);
        let mut bound = prepared.clone();
        recovery.restore(&store._installation, &mut bound, context)?;
        assert_eq!(store.load(context)?, Some(bound.clone()));
        assert_eq!(bound.revision(), prepared.revision() + 2);
        let mut discarded_result = prepared.clone();
        recovery.restore(&store._installation, &mut discarded_result, context)?;
        assert_eq!(discarded_result, bound);
        std::fs::write(store.root.path().join(super::super::RECORD), prepared.encode()?)?;
        let known = bound.clone();
        assert!(matches!(
            recovery.restore(&store._installation, &mut bound, context),
            Err(SelectionFailure::Changed)
        ));
        assert_eq!(bound, known);
        assert_eq!(store.load(context)?, Some(prepared));
        Ok(())
    })
}

#[test]
fn run_exclusion_does_not_block_preparation_and_leftover_files_do_not_lock_out() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let run = InstallationRun::acquire(Arc::clone(&store), context)?;
        let other = Arc::new(SelectionStore::open(root.path(), context)?);
        assert!(InstallationRun::acquire(Arc::clone(&other), context).is_err());
        let prepared = prepare(&store, &initial()?, 1, context)?;
        assert_eq!(other.load(context)?, Some(prepared));
        drop(run);
        let reopened = InstallationRun::acquire(other, context)?;
        drop(reopened);
        assert!(store.root.path().join(LOCK).exists());
        Ok(())
    })
}

#[test]
fn trial_token_retains_os_custody_and_cannot_be_reconstructed_from_persisted_state() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let prepared = prepare(&store, &initial()?, 1, context)?;
        let mut run = InstallationRun::acquire(Arc::clone(&store), context)?;
        let token = run.begin_trial(&prepared, context)?;
        assert!(run.begin_trial(&prepared, context).is_err());
        let persisted = store.load(context)?.ok_or("missing trial")?;
        drop(run);
        drop(store);
        let marker = root.path().join("state/installation.json");
        assert!(std::fs::write(&marker, b"changed").is_err());
        assert!(std::fs::rename(root.path(), root.path().with_extension("moved")).is_err());
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        assert!(InstallationRun::acquire(Arc::clone(&store), context).is_err());
        drop(token);
        let mut next_run = InstallationRun::acquire(Arc::clone(&store), context)?;
        assert!(next_run.begin_trial(&persisted, context).is_err());
        assert_eq!(store.load(context)?, Some(persisted));
        Ok(())
    })
}

#[test]
fn activation_requires_live_owner_and_acceptance_retains_previous_healthy_generation() -> TestResult
{
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let prepared = prepare(&store, &initial()?, 1, context)?;
        assert!(matches!(
            store.compare_exchange(&prepared, &prepared.begin_trial()?, context),
            Err(SelectionFailure::RunOwnership)
        ));
        let mut run = InstallationRun::acquire(Arc::clone(&store), context)?;
        let token = run.begin_trial(&prepared, context)?;
        let snapshot = store.load(context)?.ok_or("missing trial")?;
        assert!(matches!(
            store.compare_exchange(&snapshot, &snapshot.accept(token.attempt())?, context),
            Err(SelectionFailure::RunOwnership)
        ));
        assert!(matches!(
            store.compare_exchange(
                &snapshot,
                &snapshot.cancel_unstarted_trial(token.attempt())?,
                context
            ),
            Err(SelectionFailure::RunOwnership)
        ));
        let first = run.accept(&token, context)?;
        assert_eq!(first.accepted(), Some(&generation(1)?));
        assert!(matches!(run.accept(&token, context), Err(SelectionFailure::Changed)));
        assert_eq!(run.finish_trial(&token, BAD, context)?, first);
        drop(token);
        drop(run);
        let prepared = prepare(&store, &first, 2, context)?;
        let mut run = InstallationRun::acquire(Arc::clone(&store), context)?;
        let token = run.begin_trial(&prepared, context)?;
        assert_eq!(store.load(context)?.ok_or("missing trial")?.accepted(), first.accepted());
        let second = run.accept(&token, context)?;
        assert_eq!(second.retained(), first.accepted());
        assert_eq!(second.accepted(), Some(&generation(2)?));
        Ok(())
    })
}

#[test]
fn rejected_trial_preserves_accepted_generation_and_foreign_tokens_have_no_authority() -> TestResult
{
    worker::run(|context, _| {
        let a = root()?;
        let b = root()?;
        let store = Arc::new(
            SelectionStore::initialize(a.path(), &initial()?, context)?
                .with_strike_clock(StrikeClock::Fixed(PREPARED_AT)),
        );
        let foreign = Arc::new(SelectionStore::initialize(b.path(), &initial()?, context)?);
        let prepared = prepare(&store, &initial()?, 1, context)?;
        let mut run = InstallationRun::acquire(Arc::clone(&store), context)?;
        let token = run.begin_trial(&prepared, context)?;
        let accepted = run.accept(&token, context)?;
        drop(token);
        drop(run);
        let prepared = prepare(&store, &accepted, 2, context)?;
        let mut run = InstallationRun::acquire(Arc::clone(&store), context)?;
        let token = run.begin_trial(&prepared, context)?;
        let other = InstallationRun::acquire(foreign, context)?;
        assert!(matches!(other.accept(&token, context), Err(SelectionFailure::RunOwnership)));
        assert!(matches!(
            other.finish_trial(&token, BAD, context),
            Err(SelectionFailure::RunOwnership)
        ));
        let rejected = run.finish_trial(&token, BAD, context)?;
        assert_eq!(rejected.accepted(), accepted.accepted());
        assert_eq!(rejected.rollback(), Some(&generation(2)?));
        assert_eq!(rejected.strikes().len(), 1);
        assert_eq!(rejected.strikes()[0].package, generation(2)?.package);
        assert_eq!(rejected.strikes()[0].at, PREPARED_AT, "the store clock dates the strike");
        assert!(!rejected.withheld(&generation(2)?.package, PREPARED_AT));
        assert!(matches!(run.accept(&token, context), Err(SelectionFailure::Changed)));
        Ok(())
    })
}

#[test]
fn nonempty_run_rendezvous_is_preserved_and_never_stolen() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let run = InstallationRun::acquire(Arc::clone(&store), context)?;
        drop(run);
        let lock = store.root.path().join(LOCK);
        std::fs::write(&lock, b"unknown")?;
        assert!(InstallationRun::acquire(Arc::clone(&store), context).is_err());
        assert_eq!(std::fs::read(&lock)?, b"unknown");
        assert_eq!(store.load(context)?, Some(initial()?));
        Ok(())
    })
}

/// An indexer, backup tool or antivirus scanner reading the run lock never makes the
/// installation look owned, and a second owner is still refused while the first holds it.
#[test]
fn run_lock_tolerates_a_foreign_shared_reader_and_still_excludes_a_second_owner() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        drop(InstallationRun::acquire(Arc::clone(&store), context)?);
        let lock = store.root.path().join(LOCK);
        let foreign = File::open(&lock)?;
        let run = InstallationRun::acquire(Arc::clone(&store), context)?;
        assert!(matches!(
            InstallationRun::acquire(Arc::clone(&store), context),
            Err(SelectionFailure::RunBusy(_))
        ));
        drop(foreign);
        assert!(matches!(
            InstallationRun::acquire(Arc::clone(&store), context),
            Err(SelectionFailure::RunBusy(_))
        ));
        drop(run);
        drop(InstallationRun::acquire(Arc::clone(&store), context)?);
        assert!(std::fs::read(lock)?.is_empty());
        assert_eq!(store.load(context)?, Some(initial()?));
        Ok(())
    })
}

/// Exclusion belongs to the holder's own lock object: a duplicated witness, the only handle that
/// ever crosses processes, keeps nothing once the holder drops its run.
#[test]
fn a_duplicated_run_witness_never_retains_exclusion_after_its_holder_closes() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let run = InstallationRun::acquire(Arc::clone(&store), context)?;
        let duplicate = run.custody.lock.witness().try_clone()?;
        assert!(InstallationRun::acquire(Arc::clone(&store), context).is_err());
        drop(run);
        assert_eq!(duplicate.metadata()?.len(), 0);
        drop(InstallationRun::acquire(Arc::clone(&store), context)?);
        drop(duplicate);
        Ok(())
    })
}
