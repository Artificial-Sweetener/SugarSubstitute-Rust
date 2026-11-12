// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Walk an owned directory tree through pinned ancestry and delete only exact native objects.
//!
//! The walker bounds traversal, validates every relative name and keeps each ancestor pinned
//! while its children are visited. It never follows a reparse point: descending always opens
//! the child through [`Directory::child`], which refuses links and junctions. Owners decide
//! what each entry means; the walker grants no deletion authority of its own.
use super::{
    check,
    files::{self, Directory},
    invalid, retirement,
};
use std::{collections::BTreeSet, io, path::Path};
use sugarsubstitute_contracts::execution::JobContext;

/// Native attribute bit of a link, junction or other reparse point.
pub(crate) const REPARSE_POINT: u32 = 0x400;

/// Traversal bounds an owner applies to one walk.
#[derive(Clone, Copy)]
pub(crate) struct Limits {
    /// Most entries the walk may enumerate, counted across every depth.
    pub(crate) objects: usize,
    /// Most `/`-separated components a relative name may have.
    pub(crate) depth: usize,
}

/// What the walker does with one entry after its owner classified it.
pub(crate) enum Step {
    /// Visit the entry's children; only valid for a directory that is not a reparse point.
    Descend,
    /// Leave the entry and everything below it untouched.
    Skip,
}

/// An owner's interpretation of the entries of one walk.
pub(crate) trait Visit {
    /// Classify one enumerated entry of `parent`. `relative` is the validated `/`-separated
    /// name below the walk's root.
    /// # Errors
    /// Any error stops the walk and is returned unchanged.
    fn entry(
        &mut self,
        parent: &Directory,
        entry: &files::Entry<'_>,
        relative: &str,
        context: &JobContext,
    ) -> io::Result<Step>;

    /// Observe a descended directory after its children were visited and its pin released.
    /// `children` counts the entries it held when they were enumerated.
    /// # Errors
    /// Any error stops the walk and is returned unchanged.
    fn leave(
        &mut self,
        parent: &Directory,
        leaf: &str,
        relative: &str,
        children: usize,
        context: &JobContext,
    ) -> io::Result<()>;
}

/// Visit every entry below `root` within `limits`, descending only where the owner asks.
/// # Errors
/// Preserves native, cancellation and deadline causes and rejects exceeded bounds or names
/// that are not portable package components.
pub(crate) fn walk(
    root: &Directory,
    limits: Limits,
    visitor: &mut dyn Visit,
    context: &JobContext,
) -> io::Result<()> {
    walk_directory(root, root.path(), limits, visitor, &mut 0, context).map(|_| ())
}

fn walk_directory(
    directory: &Directory,
    root: &Path,
    limits: Limits,
    visitor: &mut dyn Visit,
    count: &mut usize,
    context: &JobContext,
) -> io::Result<usize> {
    let mut children = 0usize;
    for entry in directory.entries(context)? {
        check(context)?;
        let entry = entry?;
        children += 1;
        *count += 1;
        if *count > limits.objects {
            return Err(invalid("Storage tree exceeds inspection capacity"));
        }
        let relative = relative(&entry.path(), root)?;
        files::relative(&relative)
            .map_err(|cause| io::Error::new(io::ErrorKind::InvalidInput, cause))?;
        if relative.split('/').count() > limits.depth {
            return Err(invalid("Storage tree exceeds its directory-depth limit"));
        }
        if let Step::Descend = visitor.entry(directory, &entry, &relative, context)? {
            let leaf = leaf(&entry)?;
            let grandchildren = {
                let child = directory.child(&leaf)?;
                walk_directory(&child, root, limits, visitor, count, context)?
            };
            visitor.leave(directory, &leaf, &relative, grandchildren, context)?;
        }
    }
    Ok(children)
}

/// The `/`-separated name of `path` below `root`.
pub(crate) fn relative(path: &Path, root: &Path) -> io::Result<String> {
    Ok(path.strip_prefix(root).map_err(io::Error::other)?.to_string_lossy().replace('\\', "/"))
}

/// The entry's own name, which every owned tree spells in Unicode.
pub(crate) fn leaf(entry: &files::Entry<'_>) -> io::Result<String> {
    entry.file_name().into_string().map_err(|_| invalid("Storage entry name is not Unicode"))
}

/// A directory is owned when declared itself or an ancestor of an exact declared name.
pub(crate) fn owns_directory(allowed: &BTreeSet<String>, relative: &str) -> bool {
    let prefix = format!("{relative}/");
    allowed.contains(relative)
        || allowed.range(prefix.clone()..).next().is_some_and(|name| name.starts_with(&prefix))
}

/// Remove the now-empty owned directories below `root`, leaves first.
/// Every remaining entry must be an owned directory; anything else is an error and stays.
/// # Errors
/// Preserves native, cancellation and deadline causes and refuses changed content.
pub(crate) fn remove_owned_directories(
    root: &Path,
    allowed: &BTreeSet<String>,
    limits: Limits,
    context: &JobContext,
) -> io::Result<()> {
    let directory = match Directory::open(root, false) {
        Ok(directory) => directory,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(cause) => return Err(cause),
    };
    walk(&directory, limits, &mut OwnedDirectories { allowed }, context)
}

/// Requires that only owned directories remain and removes each after its children.
struct OwnedDirectories<'a> {
    allowed: &'a BTreeSet<String>,
}
impl Visit for OwnedDirectories<'_> {
    fn entry(
        &mut self,
        _parent: &Directory,
        entry: &files::Entry<'_>,
        relative: &str,
        _context: &JobContext,
    ) -> io::Result<Step> {
        let metadata = entry.current_metadata()?;
        if !metadata.is_dir()
            || std::os::windows::fs::MetadataExt::file_attributes(&metadata) & REPARSE_POINT != 0
            || !owns_directory(self.allowed, relative)
        {
            return Err(invalid("Directory retirement found changed content"));
        }
        Ok(Step::Descend)
    }
    fn leave(
        &mut self,
        parent: &Directory,
        leaf: &str,
        _relative: &str,
        _children: usize,
        context: &JobContext,
    ) -> io::Result<()> {
        remove_directory(&parent.path().join(leaf), context)
    }
}

/// Delete one prevalidated file while its exact parent remains pinned against replacement.
/// An absent file is already retired.
/// # Errors
/// Preserves native, cancellation and deadline causes; refuses links and changed objects.
pub(crate) fn remove_file(path: &Path, context: &JobContext) -> io::Result<()> {
    check(context)?;
    let parent =
        Directory::open(path.parent().ok_or_else(|| invalid("Deletion parent absent"))?, false)?;
    let leaf = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("Deletion leaf absent"))?;
    let file = match parent.read(leaf) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(cause) => return Err(cause),
    };
    let identity = files::file_identity(&file)?;
    drop(file);
    // The parent remains pinned. Reopen with DELETE rights and compare the exact native object.
    retirement::file(&parent, leaf, identity)
}

/// Verify an empty directory under its pinned parent before native deletion.
/// An absent directory is already retired.
/// # Errors
/// Preserves native, cancellation and deadline causes; a nonempty directory stays.
pub(crate) fn remove_directory(path: &Path, context: &JobContext) -> io::Result<()> {
    check(context)?;
    let parent =
        Directory::open(path.parent().ok_or_else(|| invalid("Deletion parent absent"))?, false)?;
    let child = match Directory::open(path, false) {
        Ok(child) => child,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(cause) => return Err(cause),
    };
    let identity = child.identity()?;
    drop(child);
    retirement::directory(
        &parent,
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid("Deletion directory leaf absent"))?,
        identity,
    )
}
