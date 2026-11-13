// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate exact inventories while pinning each ancestor; unknown files never become deletion input.
mod usage;
use super::*;
use crate::durable_storage::payload::Payload;
use std::{
    collections::{BTreeMap, BTreeSet},
    os::windows::fs::MetadataExt,
};
pub(super) use usage::{preserved_charge, scope_charge};

/// Bound physical usage and exact deletion inputs without authenticating content.
pub(super) struct Inventory {
    pub(super) bytes: u64,
    pub(super) files: Vec<String>,
    pub(super) directories: Vec<String>,
    name_bytes: usize,
    objects: usize,
    worker_records: bool,
}
impl Inventory {
    /// Admit the additional complete stage before mutation, including every retained name.
    pub(super) fn admit_record_stage(&self, name: &str) -> io::Result<()> {
        if !self.worker_records
            || self.objects >= MAX_FILES
            || self.name_bytes.checked_add(name.len() + 8).is_none_or(|bytes| bytes > 512 * 1024)
        {
            return Err(invalid("Metadata staging inventory is full"));
        }
        Ok(())
    }
}
/// Compare current contents to declared ownership before returning deletion inputs.
pub(super) fn inspect(
    directory: &Directory,
    record: &record::Record,
    context: &JobContext,
) -> io::Result<Inventory> {
    for entry in directory.entries(context)? {
        check(context)?;
        let entry = entry?;
        if ["record", "record.next", "lease"].iter().any(|name| entry.file_name() == *name) {
            let metadata = entry.current_metadata()?;
            let maximum = if entry.file_name() == "lease" { 0 } else { MAX_RECORD as u64 };
            if !metadata.is_file()
                || metadata.file_attributes() & 0x400 != 0
                || metadata.len() > maximum
            {
                return Err(invalid("Cache control exceeds its schema bound"));
            }
        }
        if !(["record", "record.next", "lease", "data"]
            .iter()
            .any(|name| entry.file_name() == *name)
            || (record.consumers.is_some() && entry.file_name() == "consumers"))
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
        worker_records: false,
    };
    let path = directory.path().join("data");
    match Directory::open(&path, false) {
        Ok(root) => visit(&root, &path, Some(&allowed), None, &mut inventory, context)?,
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
    active: Option<&BTreeMap<String, Payload>>,
    output: &mut Inventory,
    context: &JobContext,
) -> io::Result<()> {
    let mut children = 0usize;
    for entry in directory.entries(context)? {
        check(context)?;
        let entry = entry?;
        children += 1;
        output.objects += 1;
        if output.objects > if output.worker_records { MAX_FILES } else { MAX_OBJECTS } {
            return Err(invalid("Cache tree exceeds inspection capacity"));
        }
        let path = entry.path();
        let relative =
            path.strip_prefix(root).map_err(io::Error::other)?.to_string_lossy().replace('\\', "/");
        if output.worker_records {
            output.name_bytes = output
                .name_bytes
                .checked_add(relative.len() + 8)
                .ok_or_else(|| invalid("Metadata name accounting overflowed"))?;
            if output.name_bytes > 512 * 1024 {
                return Err(invalid("Metadata inventory names exceed their bound"));
            }
        }
        files::relative(&relative)
            .map_err(|cause| io::Error::new(io::ErrorKind::InvalidInput, cause))?;
        if relative.split('/').count() > MAX_DEPTH {
            return Err(invalid("Cache tree exceeds its directory-depth limit"));
        }
        let metadata = if active.is_some_and(|active| active.contains_key(&relative)) {
            entry.metadata()?
        } else {
            entry.current_metadata()?
        };
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
            visit(&child, root, allowed, active, output, context)?;
        } else if metadata.is_file() {
            if allowed.is_some_and(|allowed| !allowed.contains(&relative)) {
                return Err(invalid("Cache contains an undeclared file"));
            }
            account_name(output, &relative)?;
            let length = if let Some(payload) = active.and_then(|active| active.get(&relative)) {
                payload.length()?
            } else {
                let file = directory.read(
                    entry
                        .file_name()
                        .to_str()
                        .ok_or_else(|| invalid("Cache filename is not Unicode"))?,
                )?;
                if output.worker_records {
                    files::single_link(&file)?;
                }
                files::length(&file, context)?
            };
            output.bytes = output
                .bytes
                .checked_add(length)
                .ok_or_else(|| invalid("Cache byte accounting overflowed"))?;
            output.files.push(relative.into_boxed_str().into_string());
        } else {
            return Err(invalid("Cache tree contains an unsupported object"));
        }
    }
    if children == 0 && directory.path() != root {
        let relative = directory
            .path()
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
    if !output.worker_records {
        output.name_bytes = output
            .name_bytes
            .checked_add(relative.len())
            .ok_or_else(|| invalid("Cache inventory name accounting overflowed"))?;
    }
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
    for entry in directory.entries(context)? {
        check(context)?;
        let entry = entry?;
        let path = entry.path();
        *count += 1;
        let relative =
            path.strip_prefix(root).map_err(io::Error::other)?.to_string_lossy().replace('\\', "/");
        let metadata = entry.current_metadata()?;
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
    retirement::file(&parent, leaf, identity)
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
    retirement::directory(
        &parent,
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid("Cache directory leaf absent"))?,
        identity,
    )
}

/// Inventory a fully settled native producer's private output before declaring cleanup ownership.
pub(super) fn generated(directory: &Directory, context: &JobContext) -> io::Result<Inventory> {
    generated_with(directory, false, None, context)
}
/// Preserve delegated worker admission's 8,192-object and 512-KiB name ceilings during migration.
pub(super) fn generated_records(
    directory: &Directory,
    context: &JobContext,
) -> io::Result<Inventory> {
    generated_with(directory, true, None, context)
}
/// Inspect active generated payloads through retained share-exclusive handles, never reopening them.
pub(super) fn generated_open_records(
    directory: &Directory,
    active: &BTreeMap<String, Payload>,
    context: &JobContext,
) -> io::Result<Inventory> {
    generated_with(directory, true, Some(active), context)
}
fn generated_with(
    directory: &Directory,
    worker_records: bool,
    active: Option<&BTreeMap<String, Payload>>,
    context: &JobContext,
) -> io::Result<Inventory> {
    let path = directory.path().join("data");
    let root = Directory::open(&path, false)?;
    let mut inventory = Inventory {
        bytes: 0,
        files: Vec::new(),
        directories: Vec::new(),
        name_bytes: 0,
        objects: 0,
        worker_records,
    };
    visit(&root, &path, None, active, &mut inventory, context)?;
    Ok(inventory)
}
