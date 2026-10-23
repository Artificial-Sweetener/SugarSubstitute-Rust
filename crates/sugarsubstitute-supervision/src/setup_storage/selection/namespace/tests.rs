// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise durable namespace continuity through real protected files and admitted workers.
use super::*;
use crate::setup_storage::selection::tests::{TestResult, initial, root};
use crate::setup_storage::test_worker as worker;

#[test]
fn initialization_never_exposes_a_namespace_without_complete_initial_selection() -> TestResult {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    worker::run(|context, clock| {
        let root = root()?;
        let record = initial()?;
        let expected = record.encode()?;
        let directory = root.path().join(DIRECTORY);
        let incomplete = Arc::new(AtomicBool::new(false));
        let published = Arc::new(AtomicBool::new(false));
        let observation = (incomplete.clone(), published.clone());
        clock.observe_checks(move || {
            if directory.exists() {
                observation.1.store(true, Ordering::SeqCst);
                if !std::fs::read(directory.join(RECORD)).is_ok_and(|bytes| bytes == expected) {
                    observation.0.store(true, Ordering::SeqCst);
                }
            }
            false
        })?;
        let store = SelectionStore::initialize(root.path(), &record, context)?;
        assert!(published.load(Ordering::SeqCst));
        assert!(!incomplete.load(Ordering::SeqCst));
        assert_eq!(store.load(context)?, Some(record));
        Ok(())
    })
}

#[test]
fn enabling_selection_pins_one_immutable_marker_across_reopen() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let path = root.path().join(NAMESPACE_MARKER);
        let store = SelectionStore::initialize(root.path(), &initial()?, context)?;
        assert_eq!(std::fs::read(&path)?, CONTENT);
        assert!(std::fs::OpenOptions::new().write(true).open(&path).is_err());
        assert!(std::fs::remove_file(&path).is_err());
        drop(store);
        let store = SelectionStore::open(root.path(), context)?;
        assert_eq!(store.load(context)?, Some(initial()?));
        assert_eq!(std::fs::read(&path)?, CONTENT);
        Ok(())
    })
}

#[test]
fn missing_enabled_namespace_is_never_recreated_by_inspection_or_maintenance() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = SelectionStore::initialize(root.path(), &initial()?, context)?;
        drop(store);
        let original = root.path().join(DIRECTORY);
        let displaced = root.path().join("displaced-selection");
        let before = std::fs::read(original.join(RECORD))?;
        std::fs::rename(&original, &displaced)?;
        assert!(SelectionStore::existing(root.path(), context).is_err());
        assert!(SelectionStore::open(root.path(), context).is_err());
        assert!(!original.exists());
        assert_eq!(std::fs::read(displaced.join(RECORD))?, before);
        assert_eq!(std::fs::read(root.path().join(NAMESPACE_MARKER))?, CONTENT);
        std::fs::rename(&displaced, &original)?;
        assert_eq!(SelectionStore::open(root.path(), context)?.load(context)?, Some(initial()?));
        Ok(())
    })
}

#[test]
fn earlier_selection_is_read_without_mutation_and_enabled_without_rewriting_history() -> TestResult
{
    worker::run(|context, _| {
        let root = root()?;
        let store = SelectionStore::initialize(root.path(), &initial()?, context)?;
        drop(store);
        let marker = root.path().join(NAMESPACE_MARKER);
        std::fs::remove_file(&marker)?;
        let record = root.path().join(DIRECTORY).join(RECORD);
        let before = std::fs::read(&record)?;
        let earlier = SelectionStore::existing(root.path(), context)?.ok_or("Namespace absent")?;
        assert_eq!(earlier.load(context)?, Some(initial()?));
        assert!(!marker.exists());
        drop(earlier);
        let enabled = SelectionStore::open(root.path(), context)?;
        assert_eq!(enabled.load(context)?, Some(initial()?));
        assert_eq!(std::fs::read(&record)?, before);
        assert_eq!(std::fs::read(&marker)?, CONTENT);
        Ok(())
    })
}

#[test]
fn invalid_or_unprotected_continuity_marker_is_preserved_without_creating_state() -> TestResult {
    worker::run(|context, _| {
        for protected in [false, true] {
            let temporary = root()?;
            let directory = Directory::open(temporary.path(), false)?;
            let path = temporary.path().join(NAMESPACE_MARKER);
            let bytes: &[u8] = if protected { br#"{"schema_version":2}"# } else { CONTENT };
            if protected {
                protection::with(context, |p| {
                    let mut file = p.file(&path, context)?;
                    file.write_all(bytes)?;
                    file.sync_all()
                })?;
            } else {
                std::fs::write(&path, bytes)?;
            }
            assert!(SelectionStore::existing(directory.path(), context).is_err());
            assert!(SelectionStore::open(directory.path(), context).is_err());
            assert!(!directory.path().join(DIRECTORY).exists());
            assert_eq!(std::fs::read(path)?, bytes);
        }
        Ok(())
    })
}

#[test]
fn interrupted_marker_staging_is_not_selection_authority_and_is_preserved() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = SelectionStore::initialize(root.path(), &initial()?, context)?;
        drop(store);
        let marker = root.path().join(NAMESPACE_MARKER);
        std::fs::remove_file(&marker)?;
        let abandoned = root.path().join(DIRECTORY).join(".selection-enable-stage-interrupted");
        std::fs::write(&abandoned, b"partial marker")?;
        assert!(SelectionStore::existing(root.path(), context)?.is_some());
        assert!(!marker.exists());
        let store = SelectionStore::open(root.path(), context)?;
        assert_eq!(store.load(context)?, Some(initial()?));
        assert_eq!(std::fs::read(&marker)?, CONTENT);
        assert_eq!(std::fs::read(&abandoned)?, b"partial marker");
        Ok(())
    })
}
