// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stream closed source trees with aggregate traversal admission and retained native source pins.
use super::{
    record::{MAX_BYTES, MAX_ENTRIES, Tree},
    *,
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    os::windows::fs::MetadataExt,
};

/// The source's immutable file and directory pins outlive final activation.
pub(super) struct Snapshot {
    pub(super) tree: Tree,
    pub(super) pins: Vec<File>,
}
struct Inventory {
    discovered: u64,
    name_bytes: usize,
    entries: u64,
    bytes: u64,
    hash: Sha256,
    identities: Sha256,
    pins: Vec<File>,
}
impl Inventory {
    fn new() -> Self {
        Self {
            discovered: 0,
            name_bytes: 0,
            entries: 0,
            bytes: 0,
            hash: Sha256::new(),
            identities: Sha256::new(),
            pins: Vec::new(),
        }
    }
    /// Charge all discovered names together, including ancestors whose sibling lists remain live.
    fn admit_name(&mut self, name: &String) -> io::Result<()> {
        self.discovered =
            self.discovered.checked_add(1).ok_or_else(|| invalid("Import entry overflow"))?;
        self.name_bytes = self
            .name_bytes
            .checked_add(name.capacity())
            .and_then(|bytes| bytes.checked_add(128))
            .ok_or_else(|| invalid("Import name admission overflow"))?;
        if self.discovered > MAX_ENTRIES || self.name_bytes > 4 * 1024 * 1024 {
            return Err(invalid("Historical import exceeds aggregate traversal admission"));
        }
        Ok(())
    }
    fn pin(&mut self, file: File) -> io::Result<()> {
        if self.pins.len() as u64 >= MAX_ENTRIES {
            return Err(invalid("Historical source pin admission exceeded"));
        }
        self.pins.try_reserve(1).map_err(io::Error::other)?;
        self.pins.push(file);
        Ok(())
    }
    fn entry(&mut self, name: &str, directory: bool, length: u64) -> io::Result<()> {
        self.entries =
            self.entries.checked_add(1).ok_or_else(|| invalid("Import entry overflow"))?;
        self.bytes =
            self.bytes.checked_add(length).ok_or_else(|| invalid("Import size overflow"))?;
        if self.entries > MAX_ENTRIES || self.bytes > MAX_BYTES {
            return Err(invalid("Historical import exceeds its object or byte bound"));
        }
        self.hash.update((name.len() as u64).to_le_bytes());
        self.hash.update(name.as_bytes());
        self.hash.update([u8::from(directory)]);
        self.hash.update(length.to_le_bytes());
        Ok(())
    }
    fn identity(&mut self, identity: (u32, u32, u32)) {
        self.identities.update(identity.0.to_le_bytes());
        self.identities.update(identity.1.to_le_bytes());
        self.identities.update(identity.2.to_le_bytes());
    }
    fn finish(self) -> Snapshot {
        Snapshot {
            tree: Tree {
                entries: self.entries,
                bytes: self.bytes,
                sha256: self.hash.finalize().into(),
                identities: self.identities.finalize().into(),
            },
            pins: self.pins,
        }
    }
}

/// Copy bytes while retaining every admitted source object against replacement and file writes.
pub(super) fn copy(
    source: &Directory,
    target: &Directory,
    selection: RootEntries<'_>,
    context: &JobContext,
) -> io::Result<Snapshot> {
    let mut inventory = Inventory::new();
    visit(source, Some(target), selection, 0, &mut inventory, context)?;
    Ok(inventory.finish())
}
/// Observe the same hierarchy/content/identity stream without creating or modifying an entry.
pub(super) fn observe(
    source: &Directory,
    selection: RootEntries<'_>,
    context: &JobContext,
) -> io::Result<Snapshot> {
    let mut inventory = Inventory::new();
    visit(source, None, selection, 0, &mut inventory, context)?;
    Ok(inventory.finish())
}

fn visit(
    source: &Directory,
    target: Option<&Directory>,
    selection: RootEntries<'_>,
    depth: usize,
    inventory: &mut Inventory,
    context: &JobContext,
) -> io::Result<()> {
    check(context)?;
    if depth >= 32 {
        return Err(invalid("Historical import directory depth exceeded"));
    }
    let mut names = Vec::new();
    for entry in source.entries(context)? {
        check(context)?;
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("Historical import name is not Unicode"))?;
        files::flat_leaf(&name)?;
        if name.len() > 1024 {
            return Err(invalid("Historical import name exceeds admission"));
        }
        inventory.admit_name(&name)?;
        if depth == 0 && !selection.includes(&name) {
            continue;
        }
        let metadata = entry.current_metadata()?;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(invalid("Historical import refuses reparse points"));
        }
        names.try_reserve(1).map_err(io::Error::other)?;
        names.push(name);
    }
    names.sort();
    for name in names {
        check(context)?;
        let metadata = files::inspect(source.path().join(&name), context)?;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(invalid("Historical import refuses changed reparse points"));
        }
        inventory.entry(
            &name,
            metadata.is_dir(),
            if metadata.is_file() { metadata.len() } else { 0 },
        )?;
        if metadata.is_dir() {
            if depth == 0 && matches!(selection, RootEntries::OnlyFiles(_)) {
                return Err(invalid("Selected historical control is not an ordinary file"));
            }
            let child = source.child(&name)?;
            let identity = child.identity()?;
            inventory.identity(identity);
            inventory.pin(child.pin_leaf(context)?)?;
            match target {
                Some(target) => {
                    let path = target.path().join(&name);
                    protection::with(context, |owner| {
                        owner.directory(
                            &path,
                            super::super::protected::directories::Mode::CreateNew,
                            context,
                        )?;
                        Ok(())
                    })?;
                    let copied = target.child(&name)?;
                    visit(&child, Some(&copied), selection, depth + 1, inventory, context)?;
                }
                None => visit(&child, None, selection, depth + 1, inventory, context)?,
            }
            if child.identity()? != identity {
                return Err(invalid("Historical directory changed during import"));
            }
        } else if metadata.is_file() {
            let mut input = source.read(&name)?;
            files::single_link(&input)?;
            let identity = files::file_identity(&input)?;
            let length = files::length(&input, context)?;
            if length != metadata.len() {
                return Err(invalid("Historical file changed during inventory"));
            }
            let digest = match target {
                Some(target) => protection::with(context, |owner| {
                    let mut output = owner.file(&target.path().join(&name), context)?;
                    let written =
                        stream(&mut input, Some(&mut output), length, context).and_then(|digest| {
                            files::sync(&output, context)?;
                            Ok(digest)
                        });
                    let closed = owner.close_file(output);
                    match (written, closed) {
                        (Ok(digest), Ok(())) => Ok(digest),
                        (Err(cause), Ok(())) | (Ok(_), Err(cause)) => Err(cause),
                        (Err(operation), Err(cause)) => {
                            Err(io::Error::other(protection::DomainFailure::Native {
                                cause,
                                operation: Some(operation),
                            }))
                        }
                    }
                })?,
                None => stream(&mut input, None, length, context)?,
            };
            if files::file_identity(&input)? != identity {
                return Err(invalid("Historical file changed during import"));
            }
            inventory.identity(identity);
            inventory.hash.update(digest);
            inventory.pin(input)?;
        } else {
            return Err(invalid("Historical import requires regular files and directories"));
        }
        inventory.hash.update([0xff]);
        inventory.identities.update([0xff]);
    }
    check(context)
}

fn stream(
    input: &mut File,
    mut output: Option<&mut File>,
    length: u64,
    context: &JobContext,
) -> io::Result<[u8; 32]> {
    let mut bytes = [0; 64 * 1024];
    let mut copied = 0_u64;
    let mut hash = Sha256::new();
    loop {
        check(context)?;
        let count = input.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        copied = copied
            .checked_add(count as u64)
            .ok_or_else(|| invalid("Historical file size overflow"))?;
        if copied > length {
            return Err(invalid("Historical file grew during import"));
        }
        hash.update(&bytes[..count]);
        if let Some(output) = output.as_deref_mut() {
            check(context)?;
            output.write_all(&bytes[..count])?;
        }
    }
    if copied != length {
        return Err(invalid("Historical file changed during import"));
    }
    check(context)?;
    Ok(hash.finalize().into())
}
