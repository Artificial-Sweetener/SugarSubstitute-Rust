// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide whether an installed pack folder is a registry install that may be replaced.
//!
//! Only a folder that is itself a plain directory, is not a Git checkout and holds a valid
//! `.tracking` record is owned. Even then, a tracked file whose bytes differ from the
//! published release of the installed version is a local edit, and the folder is left alone.
//! Failing closed here is what keeps the user's work safe.

use super::digest_record::{
    FileDigests, MAX_DIGEST_RECORD_BYTES, parse_digest_record, render_digest_record,
};
use super::entry::{DIGEST_RECORD_NAME, EntryName, TRACKING_FILE_NAME};
use super::error::{ErrorCode, PackInstallError, Result};
use super::identity::{MAX_PYPROJECT_BYTES, PackIdentity};
use super::tracking::{MAX_TRACKING_BYTES, parse_tracking, tracking_bytes};
use crate::durable_storage::pack_tree::{NodeKind, PackTree};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Why a pack folder is left alone; spelled as in the shared vectors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotOwnedReason {
    /// The folder does not exist.
    Missing,
    /// The folder is a link or junction.
    LinkedFolder,
    /// The folder is a Git checkout or worktree.
    GitCheckout,
    /// The folder has no `.tracking` or no pack metadata.
    NotRegistryInstall,
    /// The `.tracking` record is invalid.
    InvalidTracking,
    /// A tracked entry is reached through a link or has an unexpected type.
    LinkedContent,
    /// A tracked file differs from the published release.
    ChangedFiles,
}

impl NotOwnedReason {
    /// Return the shared spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::LinkedFolder => "linked-folder",
            Self::GitCheckout => "git-checkout",
            Self::NotRegistryInstall => "not-registry-install",
            Self::InvalidTracking => "invalid-tracking",
            Self::LinkedContent => "linked-content",
            Self::ChangedFiles => "changed-files",
        }
    }
}

/// A registry install that may be replaced.
#[derive(Clone, Debug)]
pub(crate) struct OwnedPack {
    pub(crate) entries: Vec<EntryName>,
    pub(crate) identity: PackIdentity,
}

impl OwnedPack {
    /// Return the tracked file paths in recorded order.
    pub(crate) fn file_paths(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().filter(|entry| !entry.is_directory()).map(EntryName::path)
    }
}

/// The classification of one installed pack folder.
#[derive(Clone, Debug)]
pub(crate) enum PackOwnership {
    /// A registry install this format may replace.
    Owned(OwnedPack),
    /// A folder to leave alone, and why.
    NotOwned(NotOwnedReason),
}

/// Classify the folder ComfyUI loads the pack from, without modifying it.
pub(crate) fn inspect_pack_folder(folder: &PackTree) -> Result<PackOwnership> {
    let not_owned = |reason| Ok(PackOwnership::NotOwned(reason));
    match folder.root_kind()? {
        NodeKind::Link => return not_owned(NotOwnedReason::LinkedFolder),
        NodeKind::Missing | NodeKind::File => return not_owned(NotOwnedReason::Missing),
        NodeKind::Directory => {}
    }
    if folder.kind(".git")? != NodeKind::Missing {
        return not_owned(NotOwnedReason::GitCheckout);
    }
    let entries = match read_tracking(folder) {
        Ok(Some(entries)) => entries,
        Ok(None) => return not_owned(NotOwnedReason::NotRegistryInstall),
        Err(PackInstallError::Format { .. }) => return not_owned(NotOwnedReason::InvalidTracking),
        Err(cause) => return Err(cause),
    };
    let identity = match read_identity(folder) {
        Ok(identity) => identity,
        Err(PackInstallError::Format { .. }) => {
            return not_owned(NotOwnedReason::NotRegistryInstall);
        }
        Err(cause) => return Err(cause),
    };
    for entry in &entries {
        let kind = folder.kind(entry.path())?;
        let mismatched = match kind {
            NodeKind::Link => true,
            NodeKind::Missing => false,
            NodeKind::Directory => !entry.is_directory(),
            NodeKind::File => entry.is_directory(),
        };
        if mismatched {
            return not_owned(NotOwnedReason::LinkedContent);
        }
    }
    Ok(PackOwnership::Owned(OwnedPack { entries, identity }))
}

/// Return the folder's validated `.tracking`, or `None` when it has none.
pub(crate) fn read_tracking(folder: &PackTree) -> Result<Option<Vec<EntryName>>> {
    match folder.kind(TRACKING_FILE_NAME)? {
        NodeKind::Missing => Ok(None),
        NodeKind::File => parse_tracking(&read_tracking_bytes(folder)?).map(Some),
        NodeKind::Directory | NodeKind::Link => Err(PackInstallError::format(
            ErrorCode::TrackingUnreadable,
            "The ownership record is not a regular file.",
        )),
    }
}

fn read_tracking_bytes(folder: &PackTree) -> Result<Vec<u8>> {
    // One byte over the bound lets the parser report an oversized record as such.
    folder.read(TRACKING_FILE_NAME, MAX_TRACKING_BYTES + 1).map_err(|_| {
        PackInstallError::format(
            ErrorCode::TrackingUnreadable,
            "The ownership record could not be read.",
        )
    })
}

/// Read the identity of a folder that holds `__init__.py` and `pyproject.toml`.
pub(crate) fn read_identity(folder: &PackTree) -> Result<PackIdentity> {
    let not_pack = || {
        PackInstallError::format(ErrorCode::PackIdentity, "The folder is not a ComfyUI node pack.")
    };
    if folder.kind("__init__.py")? != NodeKind::File
        || folder.kind("pyproject.toml")? != NodeKind::File
    {
        return Err(not_pack());
    }
    let bytes = folder.read("pyproject.toml", MAX_PYPROJECT_BYTES).map_err(|_| {
        PackInstallError::format(ErrorCode::PackIdentity, "The pack metadata is unreadable.")
    })?;
    PackIdentity::parse(&bytes)
}

/// Each tracked file's SHA-256, or `None` for a tracked file now absent.
pub type InstalledDigests = BTreeMap<String, Option<String>>;

/// Return each tracked file's current digest.
pub(crate) fn installed_file_digests(
    folder: &PackTree,
    pack: &OwnedPack,
) -> Result<InstalledDigests> {
    pack.file_paths()
        .map(|path| {
            let digest = match folder.kind(path)? {
                NodeKind::File => Some(folder.sha256(path)?),
                _ => None,
            };
            Ok((path.to_owned(), digest))
        })
        .collect()
}

/// Return tracked files whose content is not the published release's, in path order.
///
/// A tracked file the release does not contain is also a change, because nothing proves it
/// came from that release. A tracked file the user deleted holds no work to lose.
pub fn changed_tracked_files(
    installed: &BTreeMap<String, Option<String>>,
    published: &FileDigests,
) -> Vec<String> {
    installed
        .iter()
        .filter_map(|(path, digest)| {
            let digest = digest.as_ref()?;
            (published.get(path) != Some(digest)).then(|| path.clone())
        })
        .collect()
}

/// Return the folder's applicable digest record, or `None`.
pub(crate) fn read_digest_record(
    folder: &PackTree,
    version: &str,
    entries: &[EntryName],
) -> Result<Option<FileDigests>> {
    if folder.kind(TRACKING_FILE_NAME)? != NodeKind::File
        || folder.kind(DIGEST_RECORD_NAME)? != NodeKind::File
    {
        return Ok(None);
    }
    let tracking = sha256_hex(&read_tracking_bytes(folder)?);
    let Ok(content) = folder.read(DIGEST_RECORD_NAME, MAX_DIGEST_RECORD_BYTES) else {
        return Ok(None);
    };
    Ok(parse_digest_record(&content, version, &tracking, entries))
}

/// Durably write `.tracking` and then the digest record bound to its exact bytes.
pub(crate) fn write_ownership_records(
    folder: &PackTree,
    entries: &[EntryName],
    version: &str,
    files: &FileDigests,
) -> Result<()> {
    let tracking = tracking_bytes(entries);
    folder.write(TRACKING_FILE_NAME, &tracking)?;
    write_digest_record(folder, version, &sha256_hex(&tracking), files)
}

/// Durably write the digest record for `.tracking` bytes with the given digest.
pub(crate) fn write_digest_record(
    folder: &PackTree,
    version: &str,
    tracking_sha256: &str,
    files: &FileDigests,
) -> Result<()> {
    let record = render_digest_record(version, tracking_sha256, files);
    folder.write(DIGEST_RECORD_NAME, record.as_bytes()).map_err(PackInstallError::from)
}

/// Return the lowercase SHA-256 of bytes.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
