// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry a bounded immutable trial snapshot without holding the mutable selection file open.
use super::*;
use crate::{
    durable_storage::{files, protection::Protection, staging},
    setup_storage::disk_inheritance::duplicate_readonly_inheritable,
};
use serde::{Deserialize, Serialize};
use std::{io::Write, os::windows::fs::FileExt};
use sugarsubstitute_contracts::installation::MAX_RECORD_BYTES;

const MAX_BYTES: usize = MAX_RECORD_BYTES * 2 + 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema_version: u32,
    identity: (u32, u32, u32),
    selection: String,
}

/// The writer and live run protect the source until its exact version has been copied.
pub(super) fn create(
    parent: &Directory,
    protection: &Protection,
    source: &File,
    record: &InstallationRecord,
    context: &JobContext,
) -> io::Result<File> {
    let snapshot = Snapshot {
        schema_version: 1,
        identity: files::file_identity(source)?,
        selection: String::from_utf8(record.encode().map_err(io::Error::other)?)
            .map_err(io::Error::other)?,
    };
    let bytes = serde_json::to_vec(&snapshot).map_err(io::Error::other)?;
    if bytes.len() > MAX_BYTES {
        return Err(crate::setup_storage::invalid("Trial snapshot exceeds its bound"));
    }
    let path = staging::unique_path(parent, ".trial-transfer")?;
    let mut file = protection.transient_file(&path, context)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    let readonly = duplicate_readonly_inheritable(&file)?;
    check(context)?;
    Ok(readonly)
}

/// Verify exact original file identity and full record, independent of shared file cursors.
pub(super) fn matches(
    file: &File,
    current: &File,
    record: &InstallationRecord,
) -> io::Result<bool> {
    let length = file.metadata()?.len();
    if length == 0 || length > MAX_BYTES as u64 {
        return Err(crate::setup_storage::invalid("Trial snapshot size is invalid"));
    }
    let mut bytes = vec![0; length as usize];
    let mut offset = 0;
    while offset < bytes.len() {
        let count = file.seek_read(&mut bytes[offset..], offset as u64)?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Trial snapshot was truncated",
            ));
        }
        offset += count;
    }
    let snapshot: Snapshot = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    if snapshot.schema_version != 1 {
        return Err(crate::setup_storage::invalid("Trial snapshot version is unsupported"));
    }
    let original =
        InstallationRecord::decode(snapshot.selection.as_bytes()).map_err(io::Error::other)?;
    Ok(snapshot.identity == files::file_identity(current)? && &original == record)
}
