// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Name, find and empty the tombstones that journal folder retirements.
use super::{
    Retired, RetirementError, RetirementOutcome, TreeInventory, busy, check, files, retirement,
    warn_preserved,
};
use crate::durable_storage::{
    files::Directory,
    staging,
    tree::{self, Limits, REPARSE_POINT, Step, Visit},
};
use std::{io, os::windows::fs::MetadataExt};
use sugarsubstitute_contracts::execution::JobContext;

/// Prefix of every tombstone name; the rest is 32 lowercase hex digits, a dot and the name.
const PREFIX: &str = ".retired-";
/// Longest name one folder entry may have, in UTF-16 units.
const MAX_NAME_UNITS: usize = 255;
/// Bounds of one tombstone walk: an application generation fits well within them.
const LIMITS: Limits = Limits { objects: 1 << 20, depth: 128 };
/// Largest record [`Tombstone::read`] returns, in bytes.
const MAX_READ_BYTES: usize = 16 * 1024 * 1024;

/// A tombstone being resumed, offered to its owner to rebuild the original inventory.
pub struct Tombstone<'a> {
    directory: &'a Directory,
    leaf: &'a str,
}

impl<'a> Tombstone<'a> {
    pub(super) fn new(directory: &'a Directory, leaf: &'a str) -> Self {
        Self { directory, leaf }
    }

    /// The folder's name before it was retired.
    pub fn leaf(&self) -> &str {
        self.leaf
    }

    /// Read the top-level regular file `name` inside the tombstone, at most `maximum` bytes,
    /// or `None` when it is absent. Links are refused.
    /// # Errors
    /// Returns `Invalid` for a name that is not one top-level file or a bound above 16 MiB,
    /// and `Storage` for an oversized, linked or unreadable file.
    pub fn read(
        &self,
        name: &str,
        maximum: usize,
        context: &JobContext,
    ) -> Result<Option<Vec<u8>>, RetirementError> {
        check(context)?;
        files::flat_leaf(name)
            .map_err(|_| RetirementError::Invalid("A tombstone record must be top-level"))?;
        if maximum > MAX_READ_BYTES {
            return Err(RetirementError::Invalid("Tombstone record bound is too large"));
        }
        match self.directory.read(name) {
            Ok(mut file) => Ok(Some(files::read_bounded(&mut file, maximum)?)),
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(cause) => Err(cause.into()),
        }
    }
}

/// Whether `leaf` has the shape of a tombstone name, compared as Windows compares names.
pub(super) fn is_tombstone(leaf: &str) -> bool {
    leaf.get(..PREFIX.len()).is_some_and(|prefix| prefix.eq_ignore_ascii_case(PREFIX))
}

/// A fresh tombstone name for `leaf`.
pub(super) fn name(leaf: &str) -> io::Result<String> {
    let name = format!("{PREFIX}{}.{leaf}", staging::identity()?);
    if name.encode_utf16().count() > MAX_NAME_UNITS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Entry name is too long to retire",
        ));
    }
    Ok(name)
}

/// The original name a tombstone name records, or `None` for any other name.
fn original(name: &str) -> Option<&str> {
    let rest = name.strip_prefix(PREFIX)?;
    let (identity, leaf) = rest.split_at_checked(32)?;
    let leaf = leaf.strip_prefix('.')?;
    (identity.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        && files::flat_leaf(leaf).is_ok())
    .then_some(leaf)
}

/// Every tombstone folder of `root`, by tombstone name and original name, in name order.
pub(super) fn list(root: &Directory, context: &JobContext) -> io::Result<Vec<(String, String)>> {
    let mut found = Vec::new();
    for entry in root.entries(context)? {
        check(context)?;
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else { continue };
        let Some(leaf) = original(&name) else { continue };
        let metadata = entry.current_metadata()?;
        if metadata.is_dir() && metadata.file_attributes() & REPARSE_POINT == 0 {
            found.push((name.clone(), leaf.to_owned()));
        }
    }
    found.sort();
    Ok(found)
}

/// Delete what `inventory` names inside the tombstone `name`, then the tombstone itself once
/// it is empty. `leaf` is the original name, used only in the report.
pub(super) fn finish(
    root: &Directory,
    name: &str,
    leaf: &str,
    inventory: &TreeInventory,
    context: &JobContext,
) -> Result<Retired, RetirementError> {
    let mut retiring = Retiring { inventory, preserved: 0, busy: 0, freed: 0 };
    {
        let directory = match root.child(name) {
            Ok(directory) => directory,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                return Ok(RetirementOutcome::Absent.into());
            }
            Err(cause) => return Err(cause.into()),
        };
        tree::walk(&directory, LIMITS, &mut retiring, context)?;
        // Final files are kept while anything is busy, so a resume can rebuild the inventory.
        if retiring.busy == 0 {
            for last in inventory.final_names() {
                retiring.file(&directory, last, context)?;
            }
        }
    }
    let Retiring { preserved, busy, freed, .. } = retiring;
    if preserved > 0 {
        warn_preserved(leaf, preserved);
    }
    let outcome = if preserved == 0 && busy == 0 {
        match retiring_empty(root, name, context)? {
            Removal::Removed => RetirementOutcome::Removed,
            Removal::Busy => RetirementOutcome::Retained { preserved, busy: 1 },
            Removal::Kept => RetirementOutcome::Retained { preserved, busy },
        }
    } else {
        RetirementOutcome::Retained { preserved, busy }
    };
    Ok(Retired { outcome, freed_bytes: freed })
}

/// How removing one emptied folder ended.
enum Removal {
    Removed,
    Busy,
    /// Something appeared in it since; that content is counted where it was found.
    Kept,
}

/// Remove the folder `leaf` of `parent` when it is empty.
fn retiring_empty(parent: &Directory, leaf: &str, context: &JobContext) -> io::Result<Removal> {
    check(context)?;
    let identity = {
        let child = match parent.child(leaf) {
            Ok(child) => child,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(Removal::Removed),
            Err(cause) => return Err(cause),
        };
        if child.entries(context)?.next().is_some() {
            return Ok(Removal::Kept);
        }
        child.identity()?
    };
    match retirement::directory(parent, leaf, identity) {
        Ok(()) => Ok(Removal::Removed),
        Err(cause) if busy(&cause) => Ok(Removal::Busy),
        // ERROR_DIR_NOT_EMPTY: content arrived after the check above.
        Err(cause) if cause.raw_os_error() == Some(145) => Ok(Removal::Kept),
        Err(cause) => Err(cause),
    }
}

/// Deletes inventoried files and emptied owned folders while counting what it keeps.
struct Retiring<'a> {
    inventory: &'a TreeInventory,
    preserved: u64,
    busy: u64,
    freed: u64,
}

impl Retiring<'_> {
    /// Delete one inventoried regular file of `parent`.
    fn file(&mut self, parent: &Directory, leaf: &str, context: &JobContext) -> io::Result<()> {
        check(context)?;
        let file = match parent.read(leaf) {
            Ok(file) => file,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(cause) if busy(&cause) => {
                self.busy += 1;
                return Ok(());
            }
            Err(cause) if cause.kind() == io::ErrorKind::InvalidInput => {
                // A link, junction or folder took the inventoried name.
                self.preserved += 1;
                return Ok(());
            }
            Err(cause) => return Err(cause),
        };
        if files::single_link(&file).is_err() {
            self.preserved += 1;
            return Ok(());
        }
        let identity = files::file_identity(&file)?;
        let length = file.metadata()?.len();
        drop(file);
        match retirement::file(parent, leaf, identity) {
            Ok(()) => {
                self.freed += length;
                Ok(())
            }
            Err(cause) if busy(&cause) => {
                self.busy += 1;
                Ok(())
            }
            Err(cause) => Err(cause),
        }
    }
}

impl Visit for Retiring<'_> {
    fn entry(
        &mut self,
        parent: &Directory,
        entry: &files::Entry<'_>,
        relative: &str,
        context: &JobContext,
    ) -> io::Result<Step> {
        let metadata = entry.current_metadata()?;
        if metadata.file_attributes() & REPARSE_POINT != 0 {
            self.preserved += 1;
            return Ok(Step::Skip);
        }
        if metadata.is_dir() {
            if tree::owns_directory(self.inventory.files(), relative) {
                return Ok(Step::Descend);
            }
            self.preserved += 1;
            return Ok(Step::Skip);
        }
        if metadata.is_file() && self.inventory.contains(relative) {
            if !self.inventory.is_final(relative) {
                self.file(parent, &tree::leaf(entry)?, context)?;
            }
            return Ok(Step::Skip);
        }
        self.preserved += 1;
        Ok(Step::Skip)
    }

    fn leave(
        &mut self,
        parent: &Directory,
        leaf: &str,
        _relative: &str,
        _children: usize,
        context: &JobContext,
    ) -> io::Result<()> {
        if let Removal::Busy = retiring_empty(parent, leaf, context)? {
            self.busy += 1;
        }
        Ok(())
    }
}
