// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real OS exclusion, independent preparation and attempt-bound publication.
use super::*;
use crate::setup_storage::{
    selection::tests::{TestResult, generation, initial, root},
    test_worker as worker,
};
use sugarsubstitute_contracts::installation::PreparationIntent;

pub(super) fn prepare(
    store: &SelectionStore,
    expected: &InstallationRecord,
    number: u64,
    context: &JobContext,
) -> Result<InstallationRecord, SelectionFailure> {
    let generation = generation(number)?;
    let staging = expected.prepare(generation.clone(), PreparationIntent::Automatic)?;
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
        assert!(matches!(run.reject(&token, context), Err(SelectionFailure::Changed)));
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
        let store = Arc::new(SelectionStore::initialize(a.path(), &initial()?, context)?);
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
        assert!(matches!(other.reject(&token, context), Err(SelectionFailure::RunOwnership)));
        let rejected = run.reject(&token, context)?;
        assert_eq!(rejected.accepted(), accepted.accepted());
        assert_eq!(rejected.rejected(), &[generation(2)?.package]);
        assert!(rejected.prepare(generation(2)?, PreparationIntent::Automatic).is_err());
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

#[test]
fn duplicated_run_handle_retains_exclusion_until_its_last_owner_closes() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let run = InstallationRun::acquire(Arc::clone(&store), context)?;
        let duplicate = run.custody._lock.try_clone()?;
        let lock = store.root.path().join(LOCK);
        assert!(File::open(&lock).is_err());
        drop(run);
        assert!(InstallationRun::acquire(Arc::clone(&store), context).is_err());
        assert!(File::open(&lock).is_err());
        assert!(std::fs::write(&lock, b"changed").is_err());
        assert!(std::fs::remove_file(&lock).is_err());
        assert!(std::fs::rename(&lock, lock.with_extension("moved")).is_err());
        assert_eq!(duplicate.metadata()?.len(), 0);
        assert_eq!(store.load(context)?, Some(initial()?));
        drop(duplicate);
        let reopened = InstallationRun::acquire(store, context)?;
        drop(reopened);
        assert!(std::fs::read(lock)?.is_empty());
        Ok(())
    })
}

#[test]
fn run_exclusion_interoperates_with_existing_byte_range_lock_owners() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let legacy = store.protected(context, |protection| {
            Ok(protection.rendezvous(&store.root.path().join(LOCK), context)?)
        })?;
        legacy.try_lock()?;
        assert!(InstallationRun::acquire(Arc::clone(&store), context).is_err());
        drop(legacy);
        let run = InstallationRun::acquire(Arc::clone(&store), context)?;
        assert!(
            store
                .protected(context, |protection| {
                    Ok(protection.rendezvous(&store.root.path().join(LOCK), context)?)
                })
                .is_err()
        );
        drop(run);
        let legacy = store.protected(context, |protection| {
            Ok(protection.rendezvous(&store.root.path().join(LOCK), context)?)
        })?;
        legacy.try_lock()?;
        Ok(())
    })
}
