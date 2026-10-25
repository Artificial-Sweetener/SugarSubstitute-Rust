// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode bounded authenticated download requests without deriving native authority from paths.
use super::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use sugarsubstitute_contracts::execution::JobContext;

/// Exact worker event envelope; unexpected fields cannot supply additional authority.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Envelope {
    pub(super) version: u32,
    pub(super) event: String,
    pub(super) id: String,
    pub(super) sequence: u64,
    pub(super) data: Request,
}
/// Command-local strictly monotonic request sequence.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub(super) version: u32,
    pub(super) serial: u64,
    operation: Operation,
}
/// Closed native operations; keys and names are bounded inert selectors within the prepared grant.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    Entry { key: String },
    Length { name: String },
    Read { name: String },
    Write { name: String, hex: String },
    Remove { name: String },
    Open { name: String, expected: Option<u64> },
    Reset { name: String },
    Chunk { name: String, offset: u64, hex: String },
    Close { name: String },
    Validate,
}
impl Request {
    /// Bound allocations and extents before invoking any native cache method.
    pub(super) fn validate(&self) -> io::Result<()> {
        let name = match &self.operation {
            Operation::Entry { key } => {
                if key.is_empty() || key.encode_utf16().count() > 1024 {
                    return Err(invalid());
                }
                None
            }
            Operation::Write { name, hex } => {
                canonical_hex(hex, 8192)?;
                Some(name)
            }
            Operation::Chunk { name, offset, hex } => {
                if *offset > 24 * 1024 * 1024 * 1024 || hex.is_empty() {
                    return Err(invalid());
                }
                canonical_hex(hex, 24 * 1024)?;
                Some(name)
            }
            Operation::Open { name, expected } => {
                if expected.is_some_and(|bytes| bytes > 24 * 1024 * 1024 * 1024) {
                    return Err(invalid());
                }
                Some(name)
            }
            Operation::Length { name }
            | Operation::Read { name }
            | Operation::Remove { name }
            | Operation::Reset { name }
            | Operation::Close { name } => Some(name),
            Operation::Validate => None,
        };
        if name.is_some_and(|name| name.is_empty() || name.len() > 4096) {
            return Err(invalid());
        }
        Ok(())
    }
    /// Feature parsing, resume and content verification stay with the upstream worker.
    pub(super) fn execute(
        self,
        access: &ProvisioningDownloads,
        active: &mut BTreeSet<String>,
        context: &JobContext,
    ) -> io::Result<Value> {
        match self.operation {
            Operation::Entry { key } => {
                let path = access.entry(&key, context)?;
                Ok(json!({"path":path.to_str().ok_or_else(invalid)?}))
            }
            Operation::Length { name } => Ok(json!({"length":access.length(&name, context)?})),
            Operation::Read { name } => {
                let record = access.read(&name, context)?;
                let hex: String =
                    record.as_slice().iter().map(|byte| format!("{byte:02x}")).collect();
                Ok(json!({"hex":hex}))
            }
            Operation::Write { name, hex } => {
                access.write(&name, &decode(&hex)?, context)?;
                Ok(json!({}))
            }
            Operation::Remove { name } => {
                access.remove(&name, context)?;
                Ok(json!({}))
            }
            Operation::Open { name, expected } => {
                let length = access.open(&name, expected, context)?;
                active.insert(name);
                Ok(json!({"length":length}))
            }
            Operation::Reset { name } => {
                access.reset(&name, context)?;
                Ok(json!({}))
            }
            Operation::Chunk { name, offset, hex } => {
                access.append(&name, offset, &decode(&hex)?, context)?;
                Ok(json!({}))
            }
            Operation::Close { name } => {
                access.close(&name)?;
                active.remove(&name);
                Ok(json!({}))
            }
            Operation::Validate => {
                access.validate(context)?;
                Ok(json!({}))
            }
        }
    }
}
/// Reject noncanonical encodings before constructing the bounded byte buffer.
fn canonical_hex(hex: &str, maximum: usize) -> io::Result<()> {
    if hex.len() > maximum * 2
        || !hex.len().is_multiple_of(2)
        || !hex.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid());
    }
    Ok(())
}
/// Decode only already validated byte pairs; the current native job owns their bounded allocation.
fn decode(hex: &str) -> io::Result<Vec<u8>> {
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let nibble = |byte: u8| match byte {
                b'0'..=b'9' => Ok(byte - b'0'),
                b'a'..=b'f' => Ok(byte - b'a' + 10),
                _ => Err(invalid()),
            };
            Ok(nibble(pair[0])? * 16 + nibble(pair[1])?)
        })
        .collect()
}
/// Product messages use the existing worker status contract; this detail is developer diagnostics.
pub(super) fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "Invalid native download operation")
}

#[cfg(test)]
mod tests;
