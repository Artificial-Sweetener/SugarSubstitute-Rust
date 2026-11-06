// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read and pin the native installation ownership marker through maintenance operations.
//!
//! Installation-owned state lives in the root's `state` folder, so the root holds only the
//! stable entry and named folders. Opening an installation first completes the move of any
//! earlier root-level state into that folder.
use super::invalid;
use crate::durable_storage::{
    files::{self, Directory},
    staging,
};
use std::{fs::File, io, path::PathBuf};
use sugarsubstitute_configuration::layout::STATE;
pub(super) const MARKER: &str = "installation.json";
pub(super) const CONTENT: &[u8] = br#"{"schema_version":1,"product":"SugarSubstitute"}"#;
/// Ownership marker of the earlier layout, which kept installation state in the root itself.
pub(super) const EARLIER_MARKER: &str = ".substitute-installation.json";
/// Earlier root-level state names and their names in the state folder. The marker moves last,
/// so an interrupted move keeps its ownership proof until every other entry has moved.
const EARLIER_STATE: [(&str, &str); 7] = [
    (".selection-state", super::selection::DIRECTORY),
    (".selection-enabled.json", super::selection::NAMESPACE_MARKER),
    (".selection-transition-lock", super::selection::TRANSITION_LOCK),
    (".update-trust", super::trust::DIRECTORY),
    (".update-trust-enabled", super::trust::CONTINUITY_MARKER),
    (".setup-lock", super::transaction::SETUP_LOCK),
    (EARLIER_MARKER, MARKER),
];

/// Path of the installation-owned state folder below `root`.
pub(super) fn state_path(root: &Directory) -> PathBuf {
    root.path().join(STATE)
}

/// Pin the existing state folder after completing any earlier-layout move.
pub(super) fn state(root: &Directory) -> io::Result<Directory> {
    complete_layout(root)?;
    Directory::open(&state_path(root), false)
}

/// Pin the state folder after completing any earlier-layout move, creating the folder when an
/// owner must place its first rendezvous in it.
pub(super) fn state_creating(root: &Directory) -> io::Result<Directory> {
    complete_layout(root)?;
    Directory::open(&state_path(root), true)
}

/// Validate the complete marker and retain its handle against replacement or modification.
pub(super) fn read(root: &Directory) -> io::Result<File> {
    let state = state(root)?;
    let mut file = state.read(MARKER)?;
    validate(&mut file)?;
    Ok(file)
}

/// Report whether `root` is owned through either layout's marker, without moving anything.
pub(super) fn present(root: &Directory) -> io::Result<bool> {
    for path in [state_path(root).join(MARKER), root.path().join(EARLIER_MARKER)] {
        match std::fs::symlink_metadata(&path) {
            Ok(_) => return Ok(true),
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
            Err(cause) => return Err(cause),
        }
    }
    Ok(false)
}

/// An unowned root may be adopted when it is empty or holds only the default setup workspace,
/// which setup itself creates in its suggested installation folder before any choice is made.
/// Any other entry belongs to someone else and blocks adoption.
pub(super) fn adoptable(
    root: &Directory,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> io::Result<bool> {
    for entry in root.entries(context)? {
        let entry = entry?;
        if !(entry.file_type()?.is_dir()
            && entry.file_name().eq_ignore_ascii_case(sugarsubstitute_configuration::layout::SETUP))
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Adopt a verified empty root: create its state folder and publish the ownership marker.
pub(super) fn establish(root: &Directory) -> io::Result<()> {
    let state = Directory::open(&state_path(root), true)?;
    files::write_new(&state.path().join(MARKER), CONTENT)
}

fn validate(file: &mut File) -> io::Result<()> {
    let value: serde_json::Value =
        serde_json::from_slice(&files::read_bounded(file, 4096)?).map_err(io::Error::other)?;
    if value != serde_json::json!({"schema_version":1,"product":"SugarSubstitute"}) {
        return Err(invalid("Installation ownership could not be verified"));
    }
    Ok(())
}

/// Move earlier root-level state into the state folder by same-volume renames.
///
/// Runs only for a root proven ours by a valid marker in either location. Each rename is
/// atomic and preserves the entry's bytes, native identity and protection, so an interrupted
/// move resumes on the next open. Names that already exist in the state folder are never
/// replaced: both copies are preserved and the conflict is reported. Unknown files, links and
/// directory junctions stay where they are.
fn complete_layout(root: &Directory) -> io::Result<()> {
    let earlier = match root.read(EARLIER_MARKER) {
        Ok(mut file) => {
            validate(&mut file)?;
            true
        }
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => false,
        Err(cause) => return Err(cause),
    };
    if !earlier && !EARLIER_STATE.iter().any(|(name, _)| occupied(&root.path().join(name))) {
        return Ok(());
    }
    if !earlier {
        // A partially moved installation proves ownership through its moved marker.
        let mut marker = Directory::open(&state_path(root), false)?.read(MARKER)?;
        validate(&mut marker)?;
    }
    let state = Directory::open(&state_path(root), true)?;
    for (name, moved) in EARLIER_STATE {
        let source = root.path().join(name);
        let metadata = match std::fs::symlink_metadata(&source) {
            Ok(metadata) => metadata,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => continue,
            Err(cause) => return Err(cause),
        };
        if metadata.file_type().is_symlink() || reparse_point(&metadata) {
            return Err(invalid(
                "Earlier installation state is a link; it has been preserved for repair",
            ));
        }
        let destination = state.path().join(moved);
        if occupied(&destination) {
            return Err(invalid(
                "Earlier and current installation state both exist; both have been preserved",
            ));
        }
        tracing::info!(
            operation = "installation_layout_move",
            entry = name,
            "Moving earlier installation state into the state folder"
        );
        if let Err(cause) = staging::publish(&source, &destination) {
            // A concurrent opener may have completed this same move first.
            if occupied(&source) || !occupied(&destination) {
                return Err(cause);
            }
        }
    }
    Ok(())
}

fn occupied(path: &std::path::Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

fn reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(test)]
mod tests;
