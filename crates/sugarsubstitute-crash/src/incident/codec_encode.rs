// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serialize canonical incident envelopes into the caller's retained output owner.
use super::{IncidentStoreError, codec::Record};
use serde::Serialize;
use std::io::Write;

/// Serialize one complete envelope without taking or replacing output custody.
/// Serialization and output failures preserve the actual original JSON cause.
pub(super) fn write<T: Serialize, W: Write>(
    schema_version: u32,
    content: &T,
    output: W,
) -> Result<(), IncidentStoreError> {
    serde_json::to_writer(output, &Record { schema_version, content })?;
    Ok(())
}

/// Own standalone canonical bytes with the existing post-serialization limit.
/// Native repository callers write directly into their admitted buffer instead.
pub(super) fn encode<T: Serialize>(
    schema_version: u32,
    content: T,
    bound: usize,
) -> Result<Vec<u8>, IncidentStoreError> {
    let mut bytes = Vec::new();
    write(schema_version, &content, &mut bytes)?;
    if bytes.len() > bound {
        return Err(IncidentStoreError::RecordBound);
    }
    Ok(bytes)
}
