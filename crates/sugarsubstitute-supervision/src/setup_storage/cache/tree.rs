// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate exact inventories while pinning each ancestor; unknown files never become deletion input.
mod usage;
use super::*;
use std::{collections::BTreeSet, os::windows::fs::MetadataExt};
pub(super) use usage::preserved_charge;

/// Bound physical usage and exact deletion inputs without authenticating content.
pub(super) struct Inventory {
    pub(super) bytes: u64,
    pub(super) files: Vec<String>,
    pub(super) directories: Vec<String>,
    name_bytes: usize,
    objects: usize,
}
/// Compare current contents to declared ownership before returning deletion inputs.
pub(super) fn inspect(
    directory: &Directory,
    record: &record::Record,
    context: &JobContext,
) -> io::Result<Inventory> {
    for entry in std::fs::read_dir(&directory.path)? {
        check(context)?;
        let entry = entry?;
        if !["record", "record.next", "lease", "data"].iter().any(|name| entry.file_name() == *name)
        {
            return Err(invalid("Cache entry contains unknown control files"));
        }
    }
    let allowed = record.inventory_names()?;
    let mut inventory = Inventory {
        bytes: 0,
        files: Vec::new(),
        directories: Vec::new(),
        name_bytes: 0,
        objects: 0,
    };
    let path = directory.path.join("data");
    match Directory::open(&path, false) {
        Ok(root) => visit(&root, &path, Some(&allowed), &mut inventory, context)?,
        Err(cause) if record.deleting && cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => return Err(cause),
    }
    Ok(inventory)
}
/// Bound traversal and pin each ancestor without following reparse points.
fn visit(
    directory: &Directory,
    root: &Path,
    allowed: Option<&BTreeSet<String>>,
    output: &mut Inventory,
    context: &JobContext,
) -> io::Result<()> {
    let mut children = 0usize;
    for entry in std::fs::read_dir(&directory.path)? {
        check(context)?;
        let entry = entry?;
        children += 1;
        output.objects += 1;
        if output.objects > MAX_OBJECTS {
            return Err(invalid("Cache tree exceeds inspection capacity"));
        }
        let path = entry.path();
        let relative =
            path.strip_prefix(root).map_err(io::Error::other)?.to_string_lossy().replace('\\', "/");
        files::relative(&relative)
            .map_err(|cause| io::Error::new(io::ErrorKind::InvalidInput, cause))?;
        if relative.split('/').count() > MAX_DEPTH {
            return Err(invalid("Cache tree exceeds its directory-depth limit"));
        }
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(invalid("Cache tree contains a link or junction"));
        }
        if metadata.is_dir() {
            if allowed.is_some_and(|allowed| !owns_directory(allowed, &relative)) {
                return Err(invalid("Cache contains an undeclared directory"));
            }
            let child = directory.child(
                entry
                    .file_name()
                    .to_str()
                    .ok_or_else(|| invalid("Cache directory name is not Unicode"))?,
            )?;
            visit(&child, root, allowed, output, context)?;
        } else if metadata.is_file() {
            if allowed.is_some_and(|allowed| !allowed.contains(&relative)) {
                return Err(invalid("Cache contains an undeclared file"));
            }
            account_name(output, &relative)?;
            let file = directory.read(
                entry
                    .file_name()
                    .to_str()
                    .ok_or_else(|| invalid("Cache filename is not Unicode"))?,
            )?;
            output.bytes = output
                .bytes
                .checked_add(file.metadata()?.len())
                .ok_or_else(|| invalid("Cache byte accounting overflowed"))?;
            output.files.push(relative.into_boxed_str().into_string());
        } else {
            return Err(invalid("Cache tree contains an unsupported object"));
        }
    }
    if children == 0 && directory.path != root {
        let relative = directory
            .path
            .strip_prefix(root)
            .map_err(io::Error::other)?
            .to_string_lossy()
            .replace('\\', "/");
        account_name(output, &relative)?;
        output.directories.push(relative.into_boxed_str().into_string());
    }
    Ok(())
}

/// Bound retained leaf names while walking implicit ancestors without duplicating them.
fn account_name(output: &mut Inventory, relative: &str) -> io::Result<()> {
    output.name_bytes = output
        .name_bytes
        .checked_add(relative.len())
        .ok_or_else(|| invalid("Cache inventory name accounting overflowed"))?;
    if output.files.len() + output.directories.len() >= MAX_FILES
        || output.name_bytes > MAX_RECORD - 1024
    {
        return Err(invalid("Cache tree names exceed inspection capacity"));
    }
    Ok(())
}
/// A directory is owned when declared empty or an ancestor of an exact declared leaf.
fn owns_directory(allowed: &BTreeSet<String>, relative: &str) -> bool {
    let prefix = format!("{relative}/");
    allowed.contains(relative)
        || allowed.range(prefix.clone()..).next().is_some_and(|name| name.starts_with(&prefix))
}

/// Remove now-empty owned ancestors without retaining duplicate long paths in memory.
pub(super) fn remove_directories(
    root: &Path,
    record: &record::Record,
    context: &JobContext,
) -> io::Result<()> {
    let directory = match Directory::open(root, false) {
        Ok(directory) => directory,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(cause) => return Err(cause),
    };
    let allowed = record.inventory_names()?;
    remove_children(&directory, root, &allowed, context, &mut 0)
}
/// Recheck every remaining child before removing only empty owned directories, from leaves up.
fn remove_children(
    directory: &Directory,
    root: &Path,
    allowed: &BTreeSet<String>,
    context: &JobContext,
    count: &mut usize,
) -> io::Result<()> {
    for entry in std::fs::read_dir(&directory.path)? {
        check(context)?;
        let path = entry?.path();
        *count += 1;
        let relative =
            path.strip_prefix(root).map_err(io::Error::other)?.to_string_lossy().replace('\\', "/");
        let metadata = std::fs::symlink_metadata(&path)?;
        if *count > MAX_OBJECTS
            || relative.split('/').count() > MAX_DEPTH
            || !metadata.is_dir()
            || metadata.file_attributes() & 0x400 != 0
            || !owns_directory(allowed, &relative)
        {
            return Err(invalid("Cache directory retirement found changed content"));
        }
        let child = directory.child(
            path.file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| invalid("Cache directory name is not Unicode"))?,
        )?;
        remove_children(&child, root, allowed, context, count)?;
        drop(child);
        remove_directory(&path, context)?;
    }
    Ok(())
}

/// Delete one prevalidated file while its exact parent remains pinned against replacement.
pub(super) fn remove_file(path: &Path, context: &JobContext) -> io::Result<()> {
    check(context)?;
    let parent = Directory::open(
        path.parent().ok_or_else(|| invalid("Cache deletion parent absent"))?,
        false,
    )?;
    let leaf = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("Cache deletion leaf absent"))?;
    let file = match parent.read(leaf) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(cause) => return Err(cause),
    };
    let identity = files::file_identity(&file)?;
    drop(file);
    // The parent remains pinned. Reopen with DELETE rights and compare the exact native object.
    super::deletion::file(&parent.path.join(leaf), identity)
}

/// Verify an empty directory under its pinned parent before native deletion.
pub(super) fn remove_directory(path: &Path, context: &JobContext) -> io::Result<()> {
    check(context)?;
    let parent = Directory::open(
        path.parent().ok_or_else(|| invalid("Cache deletion parent absent"))?,
        false,
    )?;
    let child = match Directory::open(path, false) {
        Ok(child) => child,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(cause) => return Err(cause),
    };
    let identity = child.identity()?;
    drop(child);
    super::deletion::directory(
        &parent.path.join(path.file_name().ok_or_else(|| invalid("Cache directory leaf absent"))?),
        identity,
    )
}

/// Inventory a fully settled native producer's private output before declaring cleanup ownership.
pub(super) fn generated(directory: &Directory, context: &JobContext) -> io::Result<Inventory> {
    let path = directory.path.join("data");
    let root = Directory::open(&path, false)?;
    let mut inventory = Inventory {
        bytes: 0,
        files: Vec::new(),
        directories: Vec::new(),
        name_bytes: 0,
        objects: 0,
    };
    visit(&root, &path, None, &mut inventory, context)?;
    Ok(inventory)
}
