// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A damaged or missing record, or a missing accepted release, never blocks the launch, and only
//! accepted releases start from a rebuilt record.
use super::*;
use crate::setup_storage::{
    selection::tests::{PREPARED_AT, TestResult, generation, initial, root},
    test_worker as worker,
};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use sugarsubstitute_contracts::installation::{PackageDigest, PreparationIntent};

fn record_path(root: &Path) -> PathBuf {
    root.join("state").join(DIRECTORY).join(RECORD)
}

fn sibling(root: &Path, leaf: &str) -> PathBuf {
    root.join("state").join(DIRECTORY).join(leaf)
}

/// Accept `generation` after `expected` through a real trial.
fn accept(
    store: &Arc<SelectionStore>,
    expected: &InstallationRecord,
    generation: &Generation,
    context: &JobContext,
) -> Result<InstallationRecord, SelectionFailure> {
    let staging =
        expected.prepare(generation.clone(), PreparationIntent::Automatic, PREPARED_AT)?;
    store.compare_exchange(expected, &staging, context)?;
    let prepared = staging.prepared(generation)?;
    store.compare_exchange(&staging, &prepared, context)?;
    let mut run = InstallationRun::acquire(store.clone(), context)?;
    let trial = run.begin_trial(&prepared, context)?;
    run.accept(&trial, context)
}

/// A generation whose inventory is on disk and matches its recorded package.
fn on_disk(
    root: &Path,
    number: u64,
) -> Result<Generation, Box<dyn std::error::Error + Send + Sync>> {
    let manifest = format!("inventory {number}").into_bytes();
    let generation = Generation {
        package: PackageDigest::new(format!("{:x}", Sha256::digest(&manifest)))?,
        ..generation(number)?
    };
    let directory = root.join("releases").join(generation.id.as_str());
    std::fs::create_dir_all(&directory)?;
    std::fs::write(directory.join(crate::setup_storage::inventory::INSTALLED_MANIFEST), manifest)?;
    Ok(generation)
}

/// Damage the record as `case` describes and return the bytes left in its place.
fn damage(
    root: &Path,
    case: &str,
) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
    let file = record_path(root);
    let valid = std::fs::read(&file)?;
    let damaged = match case {
        "missing" => {
            std::fs::remove_file(&file)?;
            return Ok(None);
        }
        "empty" => Vec::new(),
        "truncated" => valid[..valid.len() / 2].to_vec(),
        "garbled" => valid.iter().map(|byte| byte ^ 0x5a).collect(),
        "oversized" => vec![b' '; MAX_RECORD_BYTES + 1],
        _ => return Err(format!("Unknown damage {case}").into()),
    };
    std::fs::write(&file, &damaged)?;
    Ok(Some(damaged))
}
const DAMAGE: [&str; 5] = ["missing", "empty", "truncated", "garbled", "oversized"];

#[test]
fn every_damaged_or_missing_record_is_rebuilt_from_its_last_good_copy() -> TestResult {
    worker::run(|context, _| {
        for case in DAMAGE {
            let root = root()?;
            let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
            let first = accept(&store, &initial()?, &generation(1)?, context)?;
            let second = accept(&store, &first, &generation(2)?, context)?;
            drop(store);
            // An earlier launch read the record and kept its copy.
            assert_eq!(reconcile(root.path(), context)?, Reconciled::Readable);
            let damaged = damage(root.path(), case)?;
            assert!(SelectionStore::open(root.path(), context).is_err(), "{case}");
            assert_eq!(reconcile(root.path(), context)?, Reconciled::Restored, "{case}");
            let restored = SelectionStore::open(root.path(), context)?
                .load(context)?
                .ok_or("Absent record")?;
            assert_eq!(restored.accepted(), second.accepted(), "{case}");
            assert_eq!(restored.retained(), second.retained(), "{case}");
            assert!(restored.candidate().is_none(), "{case}");
            assert_eq!(restored.revision(), second.revision() + 1, "{case}");
            // The damaged bytes are kept exactly, for diagnosis.
            let aside = std::fs::read(sibling(root.path(), DAMAGED)).ok();
            assert_eq!(aside, damaged, "{case}");
            assert_eq!(reconcile(root.path(), context)?, Reconciled::Readable, "{case}");
        }
        Ok(())
    })
}

#[test]
fn without_a_readable_copy_a_damaged_record_is_left_exactly_as_it_is() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        accept(&store, &initial()?, &generation(1)?, context)?;
        drop(store);
        let damaged = damage(root.path(), "garbled")?.ok_or("Record absent")?;
        assert_eq!(reconcile(root.path(), context)?, Reconciled::Untouched);
        let left = std::fs::read(record_path(root.path()))?;
        assert_eq!(left, damaged);
        let aside = std::fs::read(sibling(root.path(), DAMAGED)).ok();
        assert!(aside.is_none());
        // A damaged copy is no copy.
        std::fs::write(sibling(root.path(), LAST_GOOD), b"{\"schema_version\":3")?;
        assert_eq!(reconcile(root.path(), context)?, Reconciled::Untouched);
        let left = std::fs::read(record_path(root.path()))?;
        assert_eq!(left, damaged);
        Ok(())
    })
}

#[test]
fn a_record_written_in_another_schema_is_never_treated_as_damage() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        accept(&store, &initial()?, &generation(1)?, context)?;
        drop(store);
        assert_eq!(reconcile(root.path(), context)?, Reconciled::Readable);
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(record_path(root.path()))?)?;
        value["schema_version"] = 99.into();
        let newer = serde_json::to_vec(&value)?;
        std::fs::write(record_path(root.path()), &newer)?;
        assert_eq!(reconcile(root.path(), context)?, Reconciled::Untouched);
        let left = std::fs::read(record_path(root.path()))?;
        assert_eq!(left, newer);
        Ok(())
    })
}

/// The copy was taken while an update was prepared; the rebuilt record keeps the accepted
/// release and drops the update, which the next check prepares again.
#[test]
fn a_rebuilt_record_never_starts_a_release_that_was_not_accepted() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let accepted = accept(&store, &initial()?, &generation(1)?, context)?;
        let staging =
            accepted.prepare(generation(2)?, PreparationIntent::Automatic, PREPARED_AT)?;
        store.compare_exchange(&accepted, &staging, context)?;
        let prepared = staging.prepared(&generation(2)?)?;
        store.compare_exchange(&staging, &prepared, context)?;
        drop(store);
        assert_eq!(reconcile(root.path(), context)?, Reconciled::Readable);
        damage(root.path(), "truncated")?;
        assert_eq!(reconcile(root.path(), context)?, Reconciled::Restored);
        let restored =
            SelectionStore::open(root.path(), context)?.load(context)?.ok_or("Absent record")?;
        assert_eq!(restored.accepted(), Some(&generation(1)?));
        assert!(restored.candidate().is_none());
        Ok(())
    })
}

/// The accepted release's files are gone as well as the record: the previous accepted release
/// starts and the newer one is repaired by the next update check.
#[test]
fn a_rebuilt_record_whose_accepted_release_is_gone_runs_the_previous_release() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let previous = on_disk(root.path(), 1)?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let first = accept(&store, &initial()?, &previous, context)?;
        accept(&store, &first, &generation(2)?, context)?;
        drop(store);
        assert_eq!(reconcile(root.path(), context)?, Reconciled::Readable);
        damage(root.path(), "empty")?;
        assert_eq!(reconcile(root.path(), context)?, Reconciled::Restored);
        let restored =
            SelectionStore::open(root.path(), context)?.load(context)?.ok_or("Absent record")?;
        assert_eq!(restored.accepted(), Some(&previous));
        assert_eq!(restored.repair(), Some(&generation(2)?));
        assert!(restored.strikes().is_empty());
        Ok(())
    })
}

/// A readable record names an accepted release whose files are gone. Routing runs the retained
/// release instead of refusing, while a release whose files are present is never replaced.
#[test]
fn routing_runs_the_previous_release_when_the_accepted_release_is_gone() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let previous = on_disk(root.path(), 1)?;
        let newer = on_disk(root.path(), 2)?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let first = accept(&store, &initial()?, &previous, context)?;
        let second = accept(&store, &first, &newer, context)?;
        drop(store);
        assert_eq!(route_launch(root.path(), &newer.id, 0, context)?, Route::Own);
        let store = SelectionStore::open(root.path(), context)?;
        assert_eq!(store.load(context)?, Some(second));
        drop(store);
        std::fs::remove_dir_all(root.path().join("releases").join(newer.id.as_str()))?;
        assert_eq!(route_launch(root.path(), &previous.id, 0, context)?, Route::Own);
        let fallen = SelectionStore::open(root.path(), context)?.load(context)?.ok_or("Absent")?;
        assert_eq!(fallen.accepted(), Some(&previous));
        assert_eq!(fallen.repair(), Some(&newer));
        Ok(())
    })
}

/// Routing rebuilds a damaged record before reading it, so the engine keeps startup.
#[test]
fn routing_rebuilds_a_damaged_record_and_keeps_startup() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = Arc::new(SelectionStore::initialize(root.path(), &initial()?, context)?);
        let accepted = accept(&store, &initial()?, &generation(1)?, context)?;
        drop(store);
        assert_eq!(route_launch(root.path(), &generation(1)?.id, 0, context)?, Route::Own);
        damage(root.path(), "garbled")?;
        assert_eq!(route_launch(root.path(), &generation(1)?.id, 0, context)?, Route::Own);
        let restored =
            SelectionStore::open(root.path(), context)?.load(context)?.ok_or("Absent record")?;
        assert_eq!(restored.accepted(), accepted.accepted());
        Ok(())
    })
}
