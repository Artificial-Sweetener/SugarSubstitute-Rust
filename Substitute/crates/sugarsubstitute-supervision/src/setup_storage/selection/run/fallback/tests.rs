// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fall back from a damaged accepted release under real run exclusion, then repair it.
use super::*;
use crate::setup_storage::{
    selection::{
        run::tests::prepare,
        tests::{PREPARED_AT, TestResult, generation, initial, root},
    },
    test_worker as worker,
};
use sugarsubstitute_contracts::installation::PreparationIntent;

/// Accept `number` from `expected` through a real trial.
fn install(
    store: &Arc<SelectionStore>,
    expected: &InstallationRecord,
    number: u64,
    context: &JobContext,
) -> Result<InstallationRecord, SelectionFailure> {
    let prepared = prepare(store, expected, number, context)?;
    let mut run = InstallationRun::acquire(store.clone(), context)?;
    let trial = run.begin_trial(&prepared, context)?;
    run.accept(&trial, context)
}

#[test]
fn a_damaged_release_falls_back_once_and_the_same_release_is_repaired_later() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let first = install(&store, &initial()?, 1, context)?;
        let second = install(&store, &first, 2, context)?;
        let run = InstallationRun::acquire(store.clone(), context)?;
        let fallen = run.fall_back_to_retained(&generation(2)?, context)?;
        assert_eq!(fallen.accepted(), Some(&generation(1)?));
        assert_eq!(fallen.repair(), Some(&generation(2)?));
        assert!(fallen.strikes().is_empty());
        assert!(fallen.rollback().is_none());
        assert_eq!(run.fall_back_to_retained(&generation(2)?, context)?, fallen);
        assert_eq!(store.load(context)?, Some(fallen.clone()));
        assert!(matches!(
            run.fall_back_to_retained(&generation(1)?, context),
            Err(SelectionFailure::Record(_))
        ));
        drop(run);
        // The next update check prepares the same release into a fresh directory.
        let again = Generation { id: generation(3)?.id, ..generation(2)? };
        let staging = fallen.prepare(again.clone(), PreparationIntent::Automatic, PREPARED_AT)?;
        store.compare_exchange(&fallen, &staging, context)?;
        let prepared = staging.prepared(&again)?;
        store.compare_exchange(&staging, &prepared, context)?;
        let mut run = InstallationRun::acquire(store.clone(), context)?;
        let trial = run.begin_trial(&prepared, context)?;
        let repaired = run.accept(&trial, context)?;
        assert_eq!(repaired.accepted(), Some(&again));
        assert_eq!(repaired.retained(), Some(&generation(1)?));
        assert!(repaired.repair().is_none());
        assert_ne!(repaired, second);
        Ok(())
    })
}
