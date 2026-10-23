// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Isolate mutable download bytes behind an immutable per-entry ownership declaration.
use super::DownloadFailure;
use crate::{
    protected_storage::directories::{DirectoryDisposition, Mode},
    setup_storage::{
        SelectionStore, check,
        files::{self, Directory},
        invalid, ownership, protection,
    },
};
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_update::Target;

/// Exact open payload plus the original owned directory and marker handles.
pub(super) struct Custody {
    pub(super) file: File,
    _parent: Directory,
    _cache: Directory,
    _entry: Directory,
    _installation_owner: File,
    _entry_owner: File,
    _memory: MemoryClaim,
}
impl Custody {
    /// Open only the declared payload of this installation's protected content-addressed entry.
    pub(super) fn open(
        root: &Path,
        target: &Target,
        context: &JobContext,
    ) -> Result<Self, DownloadFailure> {
        check(context)?;
        let memory = context.memory().claim(4 * 1024 * 1024).map_err(io::Error::other)?;
        let selection = SelectionStore::open(root, context)
            .and_then(|store| store.load(context))
            .map_err(io::Error::other)?
            .ok_or_else(|| invalid("Native installation selection missing"))?;
        let parent = Directory::open(root, false)?;
        let installation_owner = ownership::read(&parent)?;
        let mut declaration = b"SUDLC001".to_vec();
        declaration.extend_from_slice(selection.installation().as_str().as_bytes());
        declaration.extend_from_slice(&target.digest());
        declaration.extend_from_slice(&target.length().to_le_bytes());
        let name = target.digest().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        let (cache, entry, entry_owner, file) = protection::with(context, |protection| {
            let cache_path = parent.path.join("update-downloads-v1");
            protection.directory(&cache_path, Mode::OpenOrCreate, context)?;
            let cache = Directory::open(&cache_path, false)?;
            let entry_path = cache.path.join(name);
            let disposition = protection.directory(&entry_path, Mode::OpenOrCreate, context)?;
            let entry = Directory::open(&entry_path, false)?;
            if disposition == DirectoryDisposition::Created {
                let mut marker = protection.file(&entry.path.join("owner"), context)?;
                marker.write_all(&declaration)?;
                marker.sync_all()?;
            }
            let mut entry_owner = entry.read("owner")?;
            protection.verify_file(&entry_owner)?;
            if files::read_bounded(&mut entry_owner, declaration.len())? != declaration {
                return Err(invalid("Download entry ownership is damaged or belongs elsewhere"));
            }
            let payload = entry.path.join("payload");
            let file = match OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(0)
                .custom_flags(0x0020_0000)
                .open(&payload)
            {
                Ok(file) => file,
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                    protection.file(&payload, context)?
                }
                Err(cause) => return Err(cause),
            };
            let metadata = file.metadata()?;
            if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
                return Err(invalid("Download payload must be a regular file"));
            }
            protection.verify_file(&file)?;
            check(context)?;
            Ok((cache, entry, entry_owner, file))
        })?;
        Ok(Self {
            file,
            _parent: parent,
            _cache: cache,
            _entry: entry,
            _installation_owner: installation_owner,
            _entry_owner: entry_owner,
            _memory: memory,
        })
    }
}
