// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise settled selection through real protected storage and continuously held run custody.
use super::*;
use crate::setup_storage::{
    selection::{
        run::tests::prepare,
        tests::{TestResult, generation, initial, root},
    },
    test_worker as worker,
};

#[test]
fn unresolved_settled_trial_rejects_once_and_preserves_later_preparation() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let prepared = prepare(&store, &initial()?, 1, context)?;
        let mut first_run = InstallationRun::acquire(store.clone(), context)?;
        let first_trial = first_run.begin_trial(&prepared, context)?;
        let healthy = first_run.accept(&first_trial, context)?;
        drop(first_trial);
        drop(first_run);
        let prepared = prepare(&store, &healthy, 2, context)?;
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let trial = run.begin_trial(&prepared, context)?;
        let rejected = run.finish_trial(&trial, context)?;
        assert_eq!(rejected.accepted(), healthy.accepted());
        assert_eq!(rejected.rejected(), &[generation(2)?.package]);
        assert!(rejected.candidate().is_none());
        assert_eq!(run.finish_trial(&trial, context)?, rejected);
        let next = prepare(&store, &rejected, 3, context)?;
        assert_eq!(run.finish_trial(&trial, context)?, next);
        assert_eq!(store.load(context)?, Some(next));
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
        assert_eq!(run.finish_trial(trial, context)?, accepted);
        let next = prepare(store, &accepted, 2, context)?;
        assert_eq!(run.finish_trial(trial, context)?, next);
        assert!(next.rejected().is_empty());
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
        assert!(matches!(other.finish_trial(&trial, context), Err(SelectionFailure::RunOwnership)));
        let accepted = run.accept(&trial, context)?;
        let prepared = prepare(&store, &accepted, 2, context)?;
        // A hostile record cannot turn original custody into another attempt's authority.
        let changed = prepared.begin_trial()?;
        std::fs::write(store.root.path.join(super::super::super::RECORD), changed.encode()?)?;
        assert!(matches!(run.finish_trial(&trial, context), Err(SelectionFailure::Changed)));
        assert_eq!(store.load(context)?, Some(changed));
        Ok(())
    })
}
