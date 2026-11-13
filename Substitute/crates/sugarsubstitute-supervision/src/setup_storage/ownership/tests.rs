// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise installation ownership, root adoption and the earlier-layout state move.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult};
use std::path::Path;

fn earlier_installation(root: &Path) -> std::io::Result<()> {
    std::fs::write(root.join(EARLIER_MARKER), CONTENT)?;
    std::fs::create_dir(root.join(".selection-state"))?;
    std::fs::write(root.join(".selection-state").join("record"), b"selection")?;
    std::fs::write(root.join(".selection-enabled.json"), b"continuity")?;
    std::fs::write(root.join(".selection-transition-lock"), b"")?;
    std::fs::create_dir(root.join(".update-trust"))?;
    std::fs::write(root.join(".update-trust").join("checkpoint"), b"trust")?;
    std::fs::write(root.join(".update-trust-enabled"), b"trust continuity")?;
    std::fs::write(root.join(".setup-lock"), b"")?;
    std::fs::create_dir(root.join("releases"))?;
    std::fs::write(root.join("notes.txt"), b"user file")
}

fn identity(path: &Path) -> std::io::Result<(u32, u32, u32)> {
    files::file_identity(&File::open(path)?)
}

fn root_names(root: &Path) -> std::io::Result<Vec<String>> {
    let mut names = std::fs::read_dir(root)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<std::io::Result<Vec<_>>>()?;
    names.sort();
    Ok(names)
}

#[test]
fn established_ownership_lives_in_the_state_folder() -> std::io::Result<()> {
    let temporary = tempfile::tempdir()?;
    let root = Directory::open(temporary.path(), false)?;
    establish(&root)?;
    assert_eq!(root_names(temporary.path())?, ["state"]);
    assert_eq!(std::fs::read(temporary.path().join("state").join(MARKER))?, CONTENT);
    assert!(present(&root)?);
    read(&root)?;
    Ok(())
}

#[test]
fn earlier_root_state_moves_into_the_state_folder_preserving_bytes_and_identity()
-> std::io::Result<()> {
    let temporary = tempfile::tempdir()?;
    earlier_installation(temporary.path())?;
    let checkpoint = identity(&temporary.path().join(".update-trust").join("checkpoint"))?;
    let root = Directory::open(temporary.path(), false)?;
    assert!(present(&root)?);
    let marker = read(&root)?;
    drop(marker);
    assert_eq!(root_names(temporary.path())?, ["notes.txt", "releases", "state"]);
    let state = temporary.path().join("state");
    assert_eq!(std::fs::read(state.join(MARKER))?, CONTENT);
    assert_eq!(std::fs::read(state.join("selection").join("record"))?, b"selection");
    assert_eq!(std::fs::read(state.join("selection-enabled.json"))?, b"continuity");
    assert_eq!(std::fs::read(state.join("update-trust").join("checkpoint"))?, b"trust");
    assert_eq!(std::fs::read(state.join("update-trust-enabled"))?, b"trust continuity");
    assert!(state.join("selection-transition-lock").is_file());
    assert!(state.join("setup-lock").is_file());
    assert_eq!(identity(&state.join("update-trust").join("checkpoint"))?, checkpoint);
    assert_eq!(std::fs::read(temporary.path().join("notes.txt"))?, b"user file");
    // Reopening the moved installation changes nothing further.
    read(&root)?;
    assert_eq!(root_names(temporary.path())?, ["notes.txt", "releases", "state"]);
    Ok(())
}

#[test]
fn interrupted_move_resumes_from_the_moved_marker() -> std::io::Result<()> {
    let temporary = tempfile::tempdir()?;
    earlier_installation(temporary.path())?;
    let state = temporary.path().join("state");
    std::fs::create_dir(&state)?;
    std::fs::rename(temporary.path().join(EARLIER_MARKER), state.join(MARKER))?;
    std::fs::rename(temporary.path().join(".selection-state"), state.join("selection"))?;
    let root = Directory::open(temporary.path(), false)?;
    read(&root)?;
    assert_eq!(root_names(temporary.path())?, ["notes.txt", "releases", "state"]);
    assert_eq!(std::fs::read(state.join("update-trust").join("checkpoint"))?, b"trust");
    Ok(())
}

#[test]
fn conflicting_earlier_and_current_state_are_both_preserved() -> std::io::Result<()> {
    let temporary = tempfile::tempdir()?;
    earlier_installation(temporary.path())?;
    let state = temporary.path().join("state");
    std::fs::create_dir_all(state.join("update-trust"))?;
    std::fs::write(state.join("update-trust").join("checkpoint"), b"current")?;
    let root = Directory::open(temporary.path(), false)?;
    assert!(read(&root).is_err());
    assert_eq!(std::fs::read(temporary.path().join(".update-trust").join("checkpoint"))?, b"trust");
    assert_eq!(std::fs::read(state.join("update-trust").join("checkpoint"))?, b"current");
    assert!(temporary.path().join(EARLIER_MARKER).is_file());
    Ok(())
}

#[test]
fn unowned_or_invalid_earlier_state_is_never_moved() -> std::io::Result<()> {
    let temporary = tempfile::tempdir()?;
    std::fs::create_dir(temporary.path().join(".update-trust"))?;
    let root = Directory::open(temporary.path(), false)?;
    assert!(read(&root).is_err());
    assert_eq!(root_names(temporary.path())?, [".update-trust"]);
    std::fs::write(temporary.path().join(EARLIER_MARKER), b"{\"product\":\"other\"}")?;
    assert!(read(&root).is_err());
    assert_eq!(root_names(temporary.path())?, [".substitute-installation.json", ".update-trust"]);
    Ok(())
}

#[test]
fn only_empty_roots_or_the_default_setup_workspace_are_adoptable() -> TestResult {
    worker::run(|context, _| {
        let temporary = tempfile::tempdir()?;
        let root = Directory::open(temporary.path(), false)?;
        assert!(adoptable(&root, context)?);
        std::fs::create_dir(temporary.path().join("setup"))?;
        assert!(adoptable(&root, context)?);
        std::fs::write(temporary.path().join("user-file"), b"preserve")?;
        assert!(!adoptable(&root, context)?);
        let file_named_setup = tempfile::tempdir()?;
        std::fs::write(file_named_setup.path().join("setup"), b"not a workspace")?;
        assert!(!adoptable(&Directory::open(file_named_setup.path(), false)?, context)?);
        Ok(())
    })
}
