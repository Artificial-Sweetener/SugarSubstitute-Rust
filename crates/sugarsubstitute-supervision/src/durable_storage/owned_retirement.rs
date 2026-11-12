// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire an owner's files and folders below a root whose ownership has been verified.
//!
//! An [`OwnedRoot`] pins a folder and every ancestor for its lifetime, together with the
//! ownership proof that admitted it, so neither can be replaced while it retires anything.
//! [`OwnedRoot::retire`] removes one entry of that folder:
//!
//! - A file is deleted through an exclusive handle on the exact native object, which is atomic.
//! - A folder is first renamed to the tombstone `.retired-<32 hex digits>.<name>` beside it.
//!   The rename is atomic, never replaces anything and fails while anything inside is open,
//!   so a folder in use is never touched. The tombstone is the journal: once it exists the
//!   folder is committed to retirement, and [`OwnedRoot::resume`] finishes it after a crash.
//!
//! Only objects the owner inventoried are deleted. Links and junctions are never followed or
//! deleted. Unknown files, unknown folders and links stay in place inside the tombstone, and
//! each retirement that keeps any reports them in one warning. No other record is written.
#[cfg(test)]
mod tests;
mod tombstone;

use super::{
    check,
    files::{self, Directory},
    retirement,
    tree::REPARSE_POINT,
};
use std::{collections::BTreeSet, fmt, fs::File, io, os::windows::fs::MetadataExt};
use sugarsubstitute_contracts::execution::JobContext;
pub use tombstone::Tombstone;

/// Working memory one retirement or resume needs on its execution worker, in bytes.
/// Inventories are charged separately by the owner that builds them.
pub const RETIREMENT_RESERVATION_BYTES: u64 = 2 * 1024 * 1024;

/// A folder pinned with its ownership proof, below which its owner may retire entries.
///
/// Dropping it releases the pins; it never deletes anything by itself. Every method runs
/// synchronously on an admitted execution worker.
pub struct OwnedRoot {
    directory: Directory,
    _ownership: File,
}

impl OwnedRoot {
    /// Admit `directory` on the strength of `ownership`, a handle on the verified marker of the
    /// owner whose folder it is. The caller has checked that `directory` lies below that owner.
    pub(crate) fn verified(directory: Directory, ownership: File) -> Self {
        Self { directory, _ownership: ownership }
    }

    /// Retire the entry `leaf` of this folder, which the owner describes as `unit`.
    ///
    /// `leaf` is one child name. A [`RetirementUnit::File`] must be a regular file with one
    /// link; a [`RetirementUnit::Tree`] must be a folder. Anything else, and every link or
    /// junction, is kept and reported.
    /// # Errors
    /// Returns [`RetirementError::Stopped`] on cancellation or deadline, `Invalid` for a name
    /// that is not one child or is itself a tombstone, and `Storage` with the native cause for
    /// any other failure. An interrupted folder retirement leaves its tombstone for
    /// [`OwnedRoot::resume`].
    pub fn retire(
        &self,
        leaf: &str,
        unit: &RetirementUnit,
        context: &JobContext,
    ) -> Result<Retired, RetirementError> {
        check(context)?;
        let _memory = context
            .memory()
            .claim(RETIREMENT_RESERVATION_BYTES)
            .map_err(|cause| RetirementError::Storage(io::Error::other(cause)))?;
        if tombstone::is_tombstone(leaf) {
            return Err(RetirementError::Invalid("A tombstone is retired only by resuming it"));
        }
        files::flat_leaf(leaf).map_err(|_| RetirementError::Invalid("Name is not one child"))?;
        let handle = match retirement::open_exclusive(&self.directory, leaf) {
            Ok(handle) => handle,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                return Ok(RetirementOutcome::Absent.into());
            }
            Err(cause) if busy(&cause) => return Ok(RetirementOutcome::Busy.into()),
            Err(cause) => return Err(cause.into()),
        };
        let metadata = handle.metadata()?;
        let shape_matches = match unit {
            RetirementUnit::File => metadata.is_file() && files::single_link(&handle).is_ok(),
            RetirementUnit::Tree(_) => metadata.is_dir(),
        };
        if metadata.file_attributes() & REPARSE_POINT != 0 || !shape_matches {
            drop(handle);
            warn_preserved(leaf, 1);
            return Ok(RetirementOutcome::Retained { preserved: 1, busy: 0 }.into());
        }
        match unit {
            RetirementUnit::File => {
                let identity = files::file_identity(&handle)?;
                drop(handle);
                check(context)?;
                match retirement::file(&self.directory, leaf, identity) {
                    Ok(()) => Ok(Retired {
                        outcome: RetirementOutcome::Removed,
                        freed_bytes: metadata.len(),
                    }),
                    Err(cause) if busy(&cause) => Ok(RetirementOutcome::Busy.into()),
                    Err(cause) => Err(cause.into()),
                }
            }
            RetirementUnit::Tree(inventory) => {
                let tombstone = tombstone::name(leaf)?;
                match retirement::rename_within_parent(&handle, &tombstone) {
                    Ok(()) => {}
                    Err(cause) if busy(&cause) => return Ok(RetirementOutcome::Busy.into()),
                    Err(cause) => return Err(cause.into()),
                }
                drop(handle);
                tombstone::finish(&self.directory, &tombstone, leaf, inventory, context)
            }
        }
    }

    /// Finish every folder retirement a crash or a busy object interrupted.
    ///
    /// `inventory` describes each tombstone's original folder; it may read the tombstone's own
    /// records through [`Tombstone::read`]. Tombstones that still hold unknown content are kept
    /// and reported again. Names that are not this owner's tombstones are never touched.
    /// # Errors
    /// Returns [`RetirementError::Stopped`] on cancellation or deadline, the first error
    /// `inventory` returns, and `Storage` with the native cause for any other failure.
    pub fn resume(
        &self,
        inventory: &mut dyn FnMut(&Tombstone<'_>) -> Result<TreeInventory, RetirementError>,
        context: &JobContext,
    ) -> Result<RetirementReport, RetirementError> {
        check(context)?;
        let _memory = context
            .memory()
            .claim(RETIREMENT_RESERVATION_BYTES)
            .map_err(|cause| RetirementError::Storage(io::Error::other(cause)))?;
        let mut report = RetirementReport::default();
        for (name, leaf) in tombstone::list(&self.directory, context)? {
            let described = {
                let directory = match self.directory.child(&name) {
                    Ok(directory) => directory,
                    // Links and other objects with a tombstone's name are left alone.
                    Err(cause) if cause.kind() == io::ErrorKind::InvalidInput => continue,
                    Err(cause) => return Err(cause.into()),
                };
                inventory(&Tombstone::new(&directory, &leaf))?
            };
            let retired = tombstone::finish(&self.directory, &name, &leaf, &described, context)?;
            report.freed_bytes += retired.freed_bytes;
            match retired.outcome {
                RetirementOutcome::Removed => report.removed += 1,
                RetirementOutcome::Retained { preserved, busy } => {
                    report.retained += 1;
                    report.preserved += preserved;
                    report.busy += busy;
                }
                RetirementOutcome::Absent | RetirementOutcome::Busy => {}
            }
        }
        Ok(report)
    }
}

/// What the owner says one entry of its folder is.
pub enum RetirementUnit {
    /// One regular file, deleted only while it has a single link.
    File,
    /// A folder whose inventoried files are deleted; everything else in it is kept.
    Tree(TreeInventory),
}

/// The files of a folder its owner wrote, by `/`-separated name below that folder.
///
/// A folder is removed when it is an ancestor of an inventoried file and ends up empty.
/// Files named by [`TreeInventory::finally`] are deleted last, and only once nothing in the
/// folder is still in use, so a resumed retirement can read them to rebuild this inventory.
#[derive(Clone, Debug, Default)]
pub struct TreeInventory {
    files: BTreeSet<String>,
    finally: BTreeSet<String>,
}

impl TreeInventory {
    /// Inventory exactly `files`.
    /// # Errors
    /// Returns [`RetirementError::Invalid`] for a name that is not a portable relative path.
    pub fn new(files: impl IntoIterator<Item = String>) -> Result<Self, RetirementError> {
        let files = files
            .into_iter()
            .map(|name| {
                files::relative(&name)
                    .map(|()| name)
                    .map_err(|_| RetirementError::Invalid("Inventory name is not a relative path"))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { files, finally: BTreeSet::new() })
    }

    /// Also inventory the top-level files `names`, deleted after every other object.
    /// # Errors
    /// Returns [`RetirementError::Invalid`] for a name that is not one top-level file name.
    pub fn finally(
        mut self,
        names: impl IntoIterator<Item = String>,
    ) -> Result<Self, RetirementError> {
        for name in names {
            files::flat_leaf(&name)
                .map_err(|_| RetirementError::Invalid("A final file must be top-level"))?;
            self.files.insert(name.clone());
            self.finally.insert(name);
        }
        Ok(self)
    }

    /// Whether `relative` names an inventoried file.
    pub fn contains(&self, relative: &str) -> bool {
        self.files.contains(relative)
    }

    pub(crate) fn files(&self) -> &BTreeSet<String> {
        &self.files
    }

    pub(crate) fn is_final(&self, relative: &str) -> bool {
        self.finally.contains(relative)
    }

    pub(crate) fn final_names(&self) -> impl Iterator<Item = &str> {
        self.finally.iter().map(String::as_str)
    }
}

/// What one retirement did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Retired {
    /// How it ended.
    pub outcome: RetirementOutcome,
    /// Bytes of the inventoried files it deleted.
    pub freed_bytes: u64,
}

impl From<RetirementOutcome> for Retired {
    fn from(outcome: RetirementOutcome) -> Self {
        Self { outcome, freed_bytes: 0 }
    }
}

/// How one retirement ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetirementOutcome {
    /// Nothing by that name exists.
    Absent,
    /// The entry is in use and nothing changed. A later turn may try again.
    Busy,
    /// The entry and everything inventoried in it are gone.
    Removed,
    /// The entry is committed to retirement but content remains: `preserved` unknown objects
    /// and links that stay for good, and `busy` inventoried objects in use that
    /// [`OwnedRoot::resume`] retries.
    Retained {
        /// Unknown files and folders, links and wrongly shaped entries that are kept.
        preserved: u64,
        /// Inventoried objects that were open elsewhere.
        busy: u64,
    },
}

/// What one resume finished and what it kept.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RetirementReport {
    /// Tombstones removed completely.
    pub removed: u64,
    /// Tombstones that remain because they hold preserved or busy content.
    pub retained: u64,
    /// Unknown objects and links kept across the retained tombstones.
    pub preserved: u64,
    /// Inventoried objects still in use across the retained tombstones.
    pub busy: u64,
    /// Bytes of the inventoried files deleted.
    pub freed_bytes: u64,
}

/// Why a retirement could not run or finish.
#[derive(Debug)]
pub enum RetirementError {
    /// The root's ownership could not be verified; nothing was touched.
    Ownership(io::Error),
    /// The request was malformed; nothing was touched.
    Invalid(&'static str),
    /// The job was cancelled or reached its deadline. Work already committed resumes later.
    Stopped(io::Error),
    /// A native operation failed; its original cause is kept.
    Storage(io::Error),
}

impl fmt::Display for RetirementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ownership(_) => {
                formatter.write_str("the folder's ownership could not be verified")
            }
            Self::Invalid(message) => formatter.write_str(message),
            Self::Stopped(_) => formatter.write_str("retirement stopped before it finished"),
            Self::Storage(_) => formatter.write_str("retirement storage failed"),
        }
    }
}

impl std::error::Error for RetirementError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Ownership(cause) | Self::Stopped(cause) | Self::Storage(cause) => Some(cause),
            Self::Invalid(_) => None,
        }
    }
}

impl From<io::Error> for RetirementError {
    fn from(cause: io::Error) -> Self {
        match cause.kind() {
            io::ErrorKind::Interrupted | io::ErrorKind::TimedOut => Self::Stopped(cause),
            _ => Self::Storage(cause),
        }
    }
}

/// Whether a native failure means another handle holds the object.
pub(crate) fn busy(cause: &io::Error) -> bool {
    // Access denied, sharing violation and lock violation.
    matches!(cause.raw_os_error(), Some(5 | 32 | 33))
}

/// Report kept content once per retirement without naming any path above the entry.
fn warn_preserved(leaf: &str, preserved: u64) {
    tracing::warn!(
        operation = "storage_retirement",
        entry = leaf,
        preserved,
        "Retirement kept content it does not own"
    );
}
