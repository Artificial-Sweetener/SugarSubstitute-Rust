// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep feature-owned target verification separate from shared disk admission and eviction pins.
use super::DownloadFailure;
use crate::{
    durable_storage::{
        files::{self, Directory},
        payload::Payload,
        protection,
    },
    setup_storage::{
        SelectionStore,
        cache::{self, CacheLease, Namespace, RuntimeCache},
        check, invalid, ownership,
    },
};
use std::{
    fs::File,
    io::{self, Write},
};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_update::Target;

/// Verified download custody retains its shared-cache pin through extraction and native consumers.
pub(super) struct Custody {
    pub(super) payload: Payload,
    _entry: Directory,
    _parent: Directory,
    _installation_owner: File,
    _entry_owner: File,
    _cache: CacheLease,
}
impl Custody {
    /// Acquire shared disk admission before opening the feature-authenticated archive.
    pub(super) fn open(
        storage: RuntimeCache,
        target: &Target,
        context: &JobContext,
    ) -> Result<Self, DownloadFailure> {
        check(context)?;
        let root = storage.anchor();
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
        let key = cache::storage_key(&declaration);
        let maximum = target
            .length()
            .checked_add(declaration.len() as u64)
            .ok_or_else(|| invalid("Download cache reservation overflowed"))?;
        let mut cache = storage.acquire(
            Namespace::UpdateDownloads,
            &key,
            maximum,
            cache::utc_seconds()?,
            context,
        )?;
        cache.declare(["owner".into(), "payload".into()], context)?;
        let entry = Directory::open(&cache.path(), false)?;
        let entry_owner = protection::with(context, |protection| {
            let entry_owner = match entry.read("owner") {
                Ok(owner) => owner,
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
                    if entry.entries(context)?.next().transpose()?.is_some() {
                        return Err(invalid("Download cache contains unowned bytes"));
                    }
                    let mut marker = protection.file(&entry.path().join("owner"), context)?;
                    marker.write_all(&declaration)?;
                    files::sync(&marker, context)?;
                    drop(marker);
                    entry.read("owner")?
                }
                Err(cause) => return Err(cause),
            };
            let mut entry_owner = entry_owner;
            protection.verify_file(&entry_owner)?;
            if files::read_bounded(&mut entry_owner, declaration.len())? != declaration {
                return Err(invalid("Download entry ownership is damaged or belongs elsewhere"));
            }
            check(context)?;
            Ok(entry_owner)
        })?;
        let payload = cache.payload("payload", target.length(), context)?;
        Ok(Self {
            payload,
            _entry: entry,
            _cache: cache,
            _parent: parent,
            _installation_owner: installation_owner,
            _entry_owner: entry_owner,
        })
    }
}
