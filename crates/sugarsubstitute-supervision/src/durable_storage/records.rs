// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain selected ordinary record ancestry without granting raw handles or cleanup authority.
mod publication;
mod reusable;
use super::{
    check,
    files::{self, Directory},
    invalid,
};
pub(crate) use reusable::publish_reusable;
use std::{
    fs::ReadDir,
    io,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};

/// Maximum bytes in a worker-owned schema record, excluding the overflow sentinel.
pub const WORKER_RECORD_BYTES: usize = 65_536;
/// One inventory page stays within the authenticated transport's frame bound.
pub const RECORD_PAGE_ENTRIES: usize = 32;
/// Native ancestry retained independently of the operation that first opened it.
const DIRECTORY_BYTES: u64 = 64 * 1024;

/// Opaque selected-directory custody. Call only on an admitted finite blocking worker.
/// Feature owners retain schemas, expected revisions, and preservation decisions.
/// Dropping this object releases its native pins; it never removes files or directories.
/// Its read-only native pins can be released on any thread; filesystem operations stay off the UI.
pub struct RecordDirectory {
    root: Directory,
    inventory: Option<ReadDir>,
    visited: usize,
    writable: bool,
    _memory: MemoryClaim,
}

/// Immutable bounded bytes and their original native file/parent identity and execution reservation.
pub struct RecordBytes {
    bytes: Vec<u8>,
    identity: (u32, u32, u32),
    parent: (u32, u32, u32),
    leaf: String,
    _memory: MemoryClaim,
}
impl RecordBytes {
    /// Borrow bytes without detaching them from their original allocation custody.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
}

/// Observed child name and non-following directory classification, never traversal authority.
pub struct RecordEntry {
    /// Original Unicode leaf; opening it still validates the actual native object.
    pub name: String,
    /// True only for an observed directory that is not a symbolic link.
    pub directory: bool,
}
impl RecordDirectory {
    /// Pin one selected directory and all existing ancestors. Only its final component may be created.
    /// # Errors
    /// Rejects links, traversal, missing parents, uncharged custody, cancellation and file collisions.
    /// `create` requires `writable`; read-only inspection never creates filesystem state.
    pub fn open(
        path: &Path,
        create: bool,
        writable: bool,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        if create && !writable {
            return Err(invalid("Read-only records cannot create a directory"));
        }
        let memory = context.memory().claim(DIRECTORY_BYTES).map_err(io::Error::other)?;
        let parent = selected_parent(path)?;
        let selected =
            parent.path().join(path.file_name().ok_or_else(|| invalid("Record leaf absent"))?);
        if create {
            match std::fs::create_dir(&selected) {
                Ok(()) => {}
                Err(cause) if cause.kind() == io::ErrorKind::AlreadyExists => {}
                Err(cause) => return Err(cause),
            }
        }
        let root = Directory::open(&selected, false)?.resolved()?;
        drop(parent);
        check(context)?;
        Ok(Self { root, inventory: None, visited: 0, writable, _memory: memory })
    }
    /// Probe absence of one selected input record without following its final file link.
    /// The selected folder's own link may be followed for this read-only presence observation.
    /// This grants no managed-directory custody; a present record still requires `open`.
    /// # Errors
    /// Rejects redirected parents, ambiguous paths, nested leaves, failed observation or controls.
    pub fn record_absent(path: &Path, leaf: &str, context: &JobContext) -> io::Result<bool> {
        check(context)?;
        component(leaf)?;
        let _memory = context.memory().claim(DIRECTORY_BYTES).map_err(io::Error::other)?;
        let parent = selected_parent(path)?;
        let selected =
            parent.path().join(path.file_name().ok_or_else(|| invalid("Record leaf absent"))?);
        let absent = match std::fs::symlink_metadata(selected.join(leaf)) {
            Ok(_) => false,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => true,
            Err(cause) => return Err(cause),
        };
        check(context)?;
        Ok(absent)
    }
    /// Borrow the pinned location as an observation, without mutation or cleanup authority.
    pub fn path(&self) -> &Path {
        self.root.path()
    }

    /// Start or continue a bounded nonrecursive inventory. Unknown entries remain untouched.
    /// # Errors
    /// Rejects more than 4,096 observed entries, oversized/non-Unicode names, I/O and controls.
    /// An empty page is terminal. A new `restart` begins another independent observation.
    pub fn page(&mut self, restart: bool, context: &JobContext) -> io::Result<Vec<RecordEntry>> {
        check(context)?;
        if restart {
            self.inventory = Some(std::fs::read_dir(self.root.path())?);
            self.visited = 0;
        }
        let inventory =
            self.inventory.as_mut().ok_or_else(|| invalid("Record inventory not started"))?;
        let mut page = Vec::with_capacity(RECORD_PAGE_ENTRIES);
        for entry in inventory.take(RECORD_PAGE_ENTRIES) {
            check(context)?;
            self.visited += 1;
            if self.visited > 4096 {
                return Err(invalid("Record inventory exceeds its bound"));
            }
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| invalid("Record name is not Unicode"))?;
            if name.len() > 1024 {
                return Err(invalid("Record name exceeds its bound"));
            }
            let kind = entry.file_type()?;
            page.push(RecordEntry { name, directory: kind.is_dir() && !kind.is_symlink() });
        }
        check(context)?;
        Ok(page)
    }
    /// Read one actual regular, singly-linked record below a separately pinned optional child.
    /// # Errors
    /// Preserves native errors and rejects links, nested names, oversized records and controls.
    pub fn read(
        &self,
        child: Option<&str>,
        leaf: &str,
        context: &JobContext,
    ) -> io::Result<RecordBytes> {
        check(context)?;
        let memory =
            context.memory().claim(WORKER_RECORD_BYTES as u64 + 1025).map_err(io::Error::other)?;
        let (bytes, identity, parent) = self.with_parent(child, |parent| {
            let mut file = parent.read(leaf)?;
            files::single_link(&file)?;
            Ok((
                files::read_bounded(&mut file, WORKER_RECORD_BYTES)?,
                files::file_identity(&file)?,
                parent.identity()?,
            ))
        })?;
        check(context)?;
        Ok(RecordBytes { bytes, identity, parent, leaf: leaf.into(), _memory: memory })
    }
    /// Create an absent child directory while retaining the selected root through the effect.
    /// # Errors
    /// Rejects read-only custody, existing children, links, nested names and controls.
    pub fn create_child(&self, leaf: &str, context: &JobContext) -> io::Result<PathBuf> {
        self.write_check(context)?;
        component(leaf)?;
        let path = self.root.path().join(leaf);
        std::fs::create_dir(&path)?;
        let _child = self.root.child(leaf)?;
        check(context)?;
        Ok(path)
    }
    /// Publish complete bytes only into an absent record name.
    /// # Errors
    /// Existing content, oversized bytes and native/control errors are preserved.
    pub fn create(
        &self,
        child: Option<&str>,
        leaf: &str,
        bytes: &[u8],
        context: &JobContext,
    ) -> io::Result<()> {
        self.write_check(context)?;
        self.with_parent(child, |parent| publication::write(parent, leaf, None, bytes, context))
    }
    /// Compare the original native file/parent identity and all bytes before atomic replacement.
    /// `anchor` names an existing immutable root record, distinct from the target. All cooperating
    /// writers use this same retained anchor; its native read pin excludes alteration/replacement.
    /// The feature establishes its semantic ownership before invoking this ordinary storage port.
    /// # Errors
    /// Rejects stale bytes, read-only custody, links, busy or invalid anchors and failed publication.
    /// Interrupted staging remains intact for feature-directed recovery. No cleanup is inferred.
    pub fn replace(
        &self,
        anchor: &str,
        child: Option<&str>,
        leaf: &str,
        expected: &RecordBytes,
        bytes: &[u8],
        context: &JobContext,
    ) -> io::Result<()> {
        self.write_check(context)?;
        component(anchor)?;
        if child.is_none() && anchor == leaf {
            return Err(invalid("Replacement requires a distinct immutable anchor"));
        }
        let writer = self.root.read(anchor)?;
        files::single_link(&writer)?;
        writer.try_lock().map_err(io::Error::from)?;
        self.with_parent(child, |parent| {
            publication::write(parent, leaf, Some(expected), bytes, context)
        })
    }
    fn write_check(&self, context: &JobContext) -> io::Result<()> {
        check(context)?;
        if !self.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Record directory is read-only",
            ));
        }
        Ok(())
    }
    fn with_parent<T>(
        &self,
        child: Option<&str>,
        operation: impl FnOnce(&Directory) -> io::Result<T>,
    ) -> io::Result<T> {
        match child {
            Some(child) => {
                let pinned = self.root.child(child)?;
                operation(&pinned)
            }
            None => operation(&self.root),
        }
    }
}
/// Preserve canonical-parent selection while allowing a separate read-only final-folder probe.
fn selected_parent(path: &Path) -> io::Result<Directory> {
    files::lexical(path)?;
    let requested_parent: PathBuf =
        path.parent().ok_or_else(|| invalid("Record parent absent"))?.components().collect();
    let parent = Directory::open(&requested_parent, false)?.resolved()?;
    if parent.path().to_string_lossy().to_lowercase()
        != requested_parent.to_string_lossy().to_lowercase()
    {
        return Err(invalid("Record parent must use its resolved local location"));
    }
    Ok(parent)
}
/// Validate one Windows-safe leaf without allowing nested traversal or alternate streams.
fn component(leaf: &str) -> io::Result<()> {
    files::relative(leaf)?;
    if leaf.contains('/') {
        return Err(invalid("Record requires one child component"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
