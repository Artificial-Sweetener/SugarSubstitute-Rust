// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise first-enablement ordering with real native locks, without timing or spawned threads.
use super::*;
use crate::setup_storage::selection::tests::{TestResult, initial, root};
use crate::setup_storage::test_worker as worker;

#[test]
fn legacy_publication_excludes_first_enablement_until_its_commit_finishes() -> TestResult {
    worker::run(|context, _| {
        let temporary = root()?;
        let root = Directory::open(temporary.path(), false)?;
        legacy(&root, context, || {
            assert!(
                SelectionStore::initialize(
                    root.path(),
                    &initial().map_err(io::Error::other)?,
                    context
                )
                .is_err()
            );
            assert!(!root.path().join(DIRECTORY).exists());
            assert!(!root.path().join(NAMESPACE_MARKER).exists());
            Ok(())
        })?;
        let store = SelectionStore::initialize(root.path(), &initial()?, context)?;
        let mut published = false;
        assert!(
            legacy(&root, context, || {
                published = true;
                Ok(())
            })
            .is_err()
        );
        assert!(!published);
        assert_eq!(store.load(context)?, Some(initial()?));
        Ok(())
    })
}

#[test]
fn unfinished_enablement_excludes_publication_and_releases_after_failure() -> TestResult {
    worker::run(|context, _| {
        let temporary = root()?;
        let root = Directory::open(temporary.path(), false)?;
        protection::with(context, |protection| {
            let _transition = namespace::transition(&root, protection, context)?;
            let mut published = false;
            assert!(
                legacy(&root, context, || {
                    published = true;
                    Ok(())
                })
                .is_err()
            );
            assert!(!published);
            Ok(())
        })?;
        assert!(
            legacy(&root, context, || Err::<(), _>(io::Error::other("injected failure"))).is_err()
        );
        legacy(&root, context, || Ok(()))?;
        Ok(())
    })
}

#[test]
fn incomplete_and_damaged_selection_never_reenable_legacy_publication() -> TestResult {
    worker::run(|context, _| {
        for leaf in [DIRECTORY, NAMESPACE_MARKER, RECORD, LOCK] {
            let temporary = root()?;
            let root = Directory::open(temporary.path(), false)?;
            let path = root.path().join(leaf);
            std::fs::write(&path, b"unknown preserved state")?;
            let mut published = false;
            assert!(
                legacy(&root, context, || {
                    published = true;
                    Ok(())
                })
                .is_err()
            );
            assert!(!published);
            assert_eq!(std::fs::read(&path)?, b"unknown preserved state");
        }
        Ok(())
    })
}
