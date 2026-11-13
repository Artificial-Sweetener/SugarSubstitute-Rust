// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate exact inventories while pinning each ancestor; unknown files never become deletion input.
mod usage;
use super::*;
use crate::durable_storage::{
    payload::Payload,
    tree::{self as storage_tree, Limits, REPARSE_POINT, Step, Visit},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    os::windows::fs::MetadataExt,
};
pub(super) use usage::{preserved_charge, scope_charge};

/// Traversal bounds of an ordinary cache entry.
const ENTRY_LIMITS: Limits = Limits { objects: MAX_OBJECTS, depth: MAX_DEPTH };

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
    fn new(worker_records: bool) -> Self {
        Self {
            bytes: 0,
            files: Vec::new(),
            directories: Vec::new(),
            name_bytes: 0,
            objects: 0,
            worker_records,
        }
    }
    /// Delegated worker records keep their 8,192-object ceiling; other entries use the tree bound.
    fn limits(&self) -> Limits {
        Limits {
            objects: if self.worker_records { MAX_FILES } else { MAX_OBJECTS },
            depth: MAX_DEPTH,
        }
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
                || metadata.file_attributes() & REPARSE_POINT != 0
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
    let mut inventory = Inventory::new(false);
    let path = directory.path().join("data");
    match Directory::open(&path, false) {
        Ok(root) => walk(&root, Some(&allowed), None, &mut inventory, context)?,
        Err(cause) if record.deleting && cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => return Err(cause),
    }
    Ok(inventory)
}
/// Bound traversal and pin each ancestor without following reparse points.
fn walk(
    root: &Directory,
    allowed: Option<&BTreeSet<String>>,
    active: Option<&BTreeMap<String, Payload>>,
    output: &mut Inventory,
    context: &JobContext,
) -> io::Result<()> {
    let limits = output.limits();
    storage_tree::walk(root, limits, &mut Accounting { allowed, active, output }, context)
}

/// Accounts declared cache content; anything undeclared, linked or unsupported is an error.
struct Accounting<'a> {
    allowed: Option<&'a BTreeSet<String>>,
    active: Option<&'a BTreeMap<String, Payload>>,
    output: &'a mut Inventory,
}
impl Visit for Accounting<'_> {
    fn entry(
        &mut self,
        parent: &Directory,
        entry: &crate::durable_storage::files::Entry<'_>,
        relative: &str,
        context: &JobContext,
    ) -> io::Result<Step> {
        let output = &mut *self.output;
        output.objects += 1;
        if output.worker_records {
            output.name_bytes = output
                .name_bytes
                .checked_add(relative.len() + 8)
                .ok_or_else(|| invalid("Metadata name accounting overflowed"))?;
            if output.name_bytes > 512 * 1024 {
                return Err(invalid("Metadata inventory names exceed their bound"));
            }
        }
        let metadata = if self.active.is_some_and(|active| active.contains_key(relative)) {
            entry.metadata()?
        } else {
            entry.current_metadata()?
        };
        if metadata.file_attributes() & REPARSE_POINT != 0 {
            return Err(invalid("Cache tree contains a link or junction"));
        }
        if metadata.is_dir() {
            if self.allowed.is_some_and(|allowed| !storage_tree::owns_directory(allowed, relative))
            {
                return Err(invalid("Cache contains an undeclared directory"));
            }
            return Ok(Step::Descend);
        }
        if !metadata.is_file() {
            return Err(invalid("Cache tree contains an unsupported object"));
        }
        if self.allowed.is_some_and(|allowed| !allowed.contains(relative)) {
            return Err(invalid("Cache contains an undeclared file"));
        }
        account_name(output, relative)?;
        let length = if let Some(payload) = self.active.and_then(|active| active.get(relative)) {
            payload.length()?
        } else {
            let file = parent.read(&storage_tree::leaf(entry)?)?;
            if output.worker_records {
                files::single_link(&file)?;
            }
            files::length(&file, context)?
        };
        output.bytes = output
            .bytes
            .checked_add(length)
            .ok_or_else(|| invalid("Cache byte accounting overflowed"))?;
        output.files.push(relative.to_owned());
        Ok(Step::Skip)
    }
    fn leave(
        &mut self,
        _parent: &Directory,
        _leaf: &str,
        relative: &str,
        children: usize,
        _context: &JobContext,
    ) -> io::Result<()> {
        if children == 0 {
            account_name(self.output, relative)?;
            self.output.directories.push(relative.to_owned());
        }
        Ok(())
    }
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

/// Remove now-empty owned ancestors without retaining duplicate long paths in memory.
pub(super) fn remove_directories(
    root: &Path,
    record: &record::Record,
    context: &JobContext,
) -> io::Result<()> {
    let allowed = record.inventory_names()?;
    storage_tree::remove_owned_directories(root, &allowed, ENTRY_LIMITS, context)
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
    let mut inventory = Inventory::new(worker_records);
    walk(&root, None, active, &mut inventory, context)?;
    Ok(inventory)
}
