// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Interrupt the public initializer on each side of publication and retry with a fresh worker.
use super::*;
use crate::setup_storage::{
    selection::tests::{TestResult, initial, root},
    test_worker as worker,
};

#[test]
fn opening_absent_state_cannot_create_an_empty_authoritative_namespace() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        assert!(SelectionStore::existing(root.path(), context)?.is_none());
        assert!(matches!(
            SelectionStore::open(root.path(), context),
            Err(SelectionFailure::Missing)
        ));
        assert!(!root.path().join(DIRECTORY).exists());
        assert!(!root.path().join(NAMESPACE_MARKER).exists());
        let store = SelectionStore::initialize(root.path(), &initial()?, context)?;
        assert_eq!(store.load(context)?, Some(initial()?));
        Ok(())
    })
}

#[test]
fn interrupted_staging_leaves_no_authority_and_a_fresh_attempt_succeeds() -> TestResult {
    let root = root()?;
    let path = root.path().to_owned();
    worker::run_expiring(move |context, clock| {
        let observed = path.clone();
        clock.observe_checks(move || {
            std::fs::read_dir(&observed).is_ok_and(|entries| {
                entries.flatten().any(|entry| {
                    entry.file_name().to_string_lossy().starts_with(".selection-init-")
                })
            })
        })?;
        assert!(SelectionStore::initialize(&path, &initial()?, context).is_err());
        assert!(!path.join(DIRECTORY).exists());
        assert!(!path.join(NAMESPACE_MARKER).exists());
        Ok(())
    })?;
    let path = root.path().to_owned();
    worker::run(move |context, _| {
        let abandoned = std::fs::read_dir(&path)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<io::Result<Vec<_>>>()?;
        assert!(abandoned.iter().any(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(".selection-init-"))
        }));
        let store = SelectionStore::initialize(&path, &initial()?, context)?;
        assert_eq!(store.load(context)?, Some(initial()?));
        for path in abandoned {
            assert!(path.exists());
        }
        Ok(())
    })
}

#[test]
fn interrupted_after_directory_publication_reopens_without_resetting_identity() -> TestResult {
    let root = root()?;
    let path = root.path().to_owned();
    worker::run_expiring(move |context, clock| {
        let observed = path.clone();
        clock.observe_checks(move || {
            observed.join(DIRECTORY).join(RECORD).exists()
                && !observed.join(NAMESPACE_MARKER).exists()
        })?;
        assert!(SelectionStore::initialize(&path, &initial()?, context).is_err());
        assert_eq!(std::fs::read(path.join(DIRECTORY).join(RECORD))?, initial()?.encode()?);
        assert!(!path.join(NAMESPACE_MARKER).exists());
        Ok(())
    })?;
    let path = root.path().to_owned();
    worker::run(move |context, _| {
        assert!(SelectionStore::initialize(&path, &initial()?, context).is_err());
        let store = SelectionStore::open(&path, context)?;
        assert_eq!(store.load(context)?, Some(initial()?));
        assert!(path.join(NAMESPACE_MARKER).exists());
        assert_eq!(std::fs::read(path.join(DIRECTORY).join(RECORD))?, initial()?.encode()?);
        Ok(())
    })
}

#[test]
fn missing_authoritative_record_is_preserved_as_damage_and_cannot_be_reset() -> TestResult {
    worker::run(|context, _| {
        let root = root()?;
        let store = SelectionStore::initialize(root.path(), &initial()?, context)?;
        drop(store);
        let record = root.path().join(DIRECTORY).join(RECORD);
        let saved = root.path().join("preserved-record");
        std::fs::rename(&record, &saved)?;
        for marker_present in [true, false] {
            if !marker_present {
                std::fs::remove_file(root.path().join(NAMESPACE_MARKER))?;
            }
            assert!(SelectionStore::open(root.path(), context).is_err());
            assert!(SelectionStore::existing(root.path(), context).is_err());
            assert!(SelectionStore::initialize(root.path(), &initial()?, context).is_err());
            assert!(!record.exists());
            assert_eq!(std::fs::read(&saved)?, initial()?.encode()?);
        }
        Ok(())
    })
}
