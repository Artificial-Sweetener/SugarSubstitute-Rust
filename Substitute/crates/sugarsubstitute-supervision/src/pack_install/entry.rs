// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate the relative entry names an archive listing and a `.tracking` record may hold.
//!
//! An entry is a forward-slash relative path; a trailing slash marks a directory, as in a
//! zip listing. Every rule is lexical and checked before any filesystem access, on every
//! platform, so a pack stays installable on a case-insensitive Windows volume.

use super::error::{ErrorCode, PackInstallError, Result};
use std::collections::BTreeSet;

/// The ownership record a registry install keeps at the pack root.
pub const TRACKING_FILE_NAME: &str = ".tracking";
/// The digest record this format keeps beside `.tracking`.
pub const DIGEST_RECORD_NAME: &str = ".tracking-digests.json";
const MAX_ENTRY_CHARACTERS: usize = 512;
const FORBIDDEN_CHARACTERS: &[char] = &['\\', ':', '*', '?', '"', '<', '>', '|'];

/// One validated entry and whether it names a directory.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntryName {
    text: String,
    directory: bool,
}

impl EntryName {
    /// Return the entry exactly as listed, including a directory's trailing slash.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Return whether the entry names a directory.
    pub fn is_directory(&self) -> bool {
        self.directory
    }

    /// Return the entry without its directory marker.
    pub fn path(&self) -> &str {
        self.text.trim_end_matches('/')
    }

    /// Return the folded path used to detect collisions on a case-insensitive volume.
    pub(crate) fn folded(&self) -> String {
        fold(self.path())
    }

    /// Return the path's segments.
    pub(crate) fn segments(&self) -> impl Iterator<Item = &str> {
        self.path().split('/')
    }
}

/// Fold a path the way the shared format compares names.
pub(crate) fn fold(path: &str) -> String {
    path.to_lowercase()
}

/// Return one validated entry or the shared typed refusal.
///
/// # Errors
/// Refuses empty, absolute, drive, backslash, stream, dot, empty-segment, control-character,
/// trailing-dot or trailing-space names (`unsafe-path`), and Windows device names, `.git`
/// metadata and the ownership records themselves (`reserved-name`).
pub fn parse_entry(raw: &str) -> Result<EntryName> {
    if raw.is_empty() || raw.chars().count() > MAX_ENTRY_CHARACTERS || raw.trim() != raw {
        return Err(unsafe_path());
    }
    let directory = raw.ends_with('/');
    let body = if directory { &raw[..raw.len() - 1] } else { raw };
    if body.is_empty() || body.starts_with('/') {
        return Err(unsafe_path());
    }
    for segment in body.split('/') {
        validate_segment(segment)?;
    }
    if !directory && (body == TRACKING_FILE_NAME || body == DIGEST_RECORD_NAME) {
        return Err(PackInstallError::format(
            ErrorCode::ReservedName,
            "An archive may not carry its own ownership record.",
        ));
    }
    Ok(EntryName { text: raw.to_owned(), directory })
}

fn validate_segment(segment: &str) -> Result<()> {
    if matches!(segment, "" | "." | "..")
        || segment.chars().any(|character| (character as u32) < 32)
        || segment.contains(FORBIDDEN_CHARACTERS)
        || segment.ends_with(['.', ' '])
    {
        return Err(unsafe_path());
    }
    if fold(segment) == ".git" {
        return Err(PackInstallError::format(
            ErrorCode::ReservedName,
            "An archive may not carry Git metadata.",
        ));
    }
    let stem = fold(segment.split('.').next().unwrap_or(segment));
    let device = matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || ["com", "lpt"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|rest| {
                let mut characters = rest.chars();
                matches!(
                    (characters.next(), characters.next()),
                    (Some('0'..='9' | '¹' | '²' | '³'), None)
                )
            })
        });
    if device {
        return Err(PackInstallError::format(
            ErrorCode::ReservedName,
            "An entry uses a name Windows reserves for devices.",
        ));
    }
    Ok(())
}

fn unsafe_path() -> PackInstallError {
    PackInstallError::format(ErrorCode::UnsafePath, "An entry name is unsafe or not portable.")
}

/// Refuse duplicate or contradictory entries in one listing.
///
/// # Errors
/// Returns `duplicate-entry` for a case-insensitive repeat and `entry-conflict` for an entry
/// inside a file entry.
pub fn validate_entry_set(entries: &[EntryName]) -> Result<()> {
    let mut files = BTreeSet::new();
    let mut directories = BTreeSet::new();
    for entry in entries {
        let key = entry.folded();
        if files.contains(&key) || (!entry.directory && directories.contains(&key)) {
            return Err(PackInstallError::format(
                ErrorCode::DuplicateEntry,
                "An entry appears more than once.",
            ));
        }
        if entry.directory {
            directories.insert(key);
        } else {
            files.insert(key);
        }
    }
    for entry in entries {
        let folded = entry.folded();
        let parts: Vec<&str> = folded.split('/').collect();
        if (1..parts.len()).any(|depth| files.contains(&parts[..depth].join("/"))) {
            return Err(PackInstallError::format(
                ErrorCode::EntryConflict,
                "An entry sits inside a file entry.",
            ));
        }
    }
    Ok(())
}
