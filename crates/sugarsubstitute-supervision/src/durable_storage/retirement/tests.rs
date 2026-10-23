// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify exact native identity, retained ancestry and conservative retirement.
use super::*;
use crate::setup_storage::test_worker::{self, TestResult};

#[test]
fn retirement_rejects_changed_identity_links_and_paths_outside_retained_parent() -> TestResult {
    test_worker::run(|_, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        let path = parent.path().join("record");
        std::fs::write(&path, b"preserved")?;
        let original = parent.read("record")?;
        let identity = files::file_identity(&original)?;
        drop(original);
        let wrong = (identity.0, identity.1, identity.2 ^ 1);
        assert!(file(&parent, "record", wrong).is_err());
        assert!(file(&parent, "../record", identity).is_err());
        assert!(file(&parent, "nested/record", identity).is_err());
        std::fs::hard_link(&path, parent.path().join("authored-link"))?;
        assert!(file(&parent, "record", identity).is_err());
        assert_eq!(std::fs::read(&path)?, b"preserved");
        assert_eq!(std::fs::read(parent.path().join("authored-link"))?, b"preserved");
        std::fs::remove_file(parent.path().join("authored-link"))?;
        let reader = parent.read("record")?;
        assert!(file(&parent, "record", identity).is_err());
        drop(reader);
        file(&parent, "record", identity)?;
        assert!(!path.exists());
        Ok(())
    })
}

#[test]
fn directory_retirement_requires_original_empty_object_and_released_child_pin() -> TestResult {
    test_worker::run(|_, _| {
        let temporary = tempfile::tempdir()?;
        let parent = Directory::open(temporary.path(), false)?;
        let path = parent.path().join("generation");
        std::fs::create_dir(&path)?;
        let child = parent.child("generation")?;
        let identity = child.identity()?;
        assert!(directory(&parent, "generation", identity).is_err());
        drop(child);
        std::fs::write(path.join("unknown"), b"preserved")?;
        assert!(directory(&parent, "generation", identity).is_err());
        assert_eq!(std::fs::read(path.join("unknown"))?, b"preserved");
        std::fs::remove_file(path.join("unknown"))?;
        directory(&parent, "generation", identity)?;
        assert!(!path.exists());
        Ok(())
    })
}
