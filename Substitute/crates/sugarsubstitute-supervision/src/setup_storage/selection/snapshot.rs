// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry an immutable selection value to another process instead of holding the record open.
//!
//! A snapshot is a private delete-on-close file holding the exact record bytes read under the
//! selection lock and the purpose it was made for. It never blocks record replacement. A receiver
//! compares the value with the current record under the lock at its point of use.
use super::*;
use crate::setup_storage::disk_inheritance::duplicate_readonly_inheritable;
use serde::{Deserialize, Serialize};
use std::os::windows::fs::FileExt;

const SCHEMA: u32 = 2;
const MAX_BYTES: usize = MAX_RECORD_BYTES * 2 + 1024;

/// Why a snapshot was handed over; a receiver accepts only the purpose it expects.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Purpose {
    /// An accepted launch reserved by the stable launcher for the elected coordinator.
    Launch,
    /// A live trial's invocation forwarded to the running coordinator; never primary admission.
    Forward,
    /// The original trial transferred with its run lock to the custody helper.
    Trial,
}
impl Purpose {
    const fn prefix(self) -> &'static str {
        match self {
            Self::Launch => ".selection-launch",
            Self::Forward => ".selection-forward",
            Self::Trial => ".trial-transfer",
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema_version: u32,
    purpose: Purpose,
    /// Native identity of the installation directory whose record was read; a record value
    /// from another installation never matches, even with equal contents.
    installation_directory: (u32, u32, u32),
    selection: String,
}

/// Write `record` to a fresh protected snapshot and return its read-only inheritable handle.
/// Delete-on-close removes the file once the last handle, including the receiver's, closes.
pub(super) fn create(
    store: &SelectionStore,
    protection: &Protection,
    record: &InstallationRecord,
    purpose: Purpose,
    context: &JobContext,
) -> io::Result<File> {
    let selection =
        String::from_utf8(record.encode().map_err(io::Error::other)?).map_err(io::Error::other)?;
    let bytes = serde_json::to_vec(&Snapshot {
        schema_version: SCHEMA,
        purpose,
        installation_directory: store._installation.identity()?,
        selection,
    })
    .map_err(io::Error::other)?;
    if bytes.len() > MAX_BYTES {
        return Err(crate::setup_storage::invalid("Selection snapshot exceeds its bound"));
    }
    let path = staging::unique_path(&store.root, purpose.prefix())?;
    let mut file = protection.transient_file(&path, context)?;
    file.write_all(&bytes)?;
    files::sync(&file, context)?;
    let readonly = duplicate_readonly_inheritable(&file)?;
    check(context)?;
    Ok(readonly)
}

/// Read a received snapshot independently of any shared file cursor.
/// # Errors
/// Rejects a foreign-protected, oversized, truncated or unsupported snapshot; one made for a
/// different purpose or another installation is `Changed`.
pub(super) fn read(
    store: &SelectionStore,
    file: &File,
    expected: Purpose,
    protection: &Protection,
    context: &JobContext,
) -> Result<InstallationRecord, SelectionFailure> {
    protection.verify_file(file)?;
    let length = files::length(file, context)?;
    if length == 0 || length > MAX_BYTES as u64 {
        return Err(crate::setup_storage::invalid("Selection snapshot size is invalid").into());
    }
    let mut bytes = vec![0; length as usize];
    let mut offset = 0;
    while offset < bytes.len() {
        check(context)?;
        let count = file.seek_read(&mut bytes[offset..], offset as u64)?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Selection snapshot was truncated",
            )
            .into());
        }
        offset += count;
    }
    let snapshot: Snapshot = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    if snapshot.schema_version != SCHEMA {
        return Err(
            crate::setup_storage::invalid("Selection snapshot version is unsupported").into()
        );
    }
    if snapshot.purpose != expected
        || snapshot.installation_directory != store._installation.identity()?
    {
        return Err(SelectionFailure::Changed);
    }
    Ok(InstallationRecord::decode(snapshot.selection.as_bytes())?)
}
