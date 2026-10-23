// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shared native distribution inventory; decoding grants no signature or execution authority.
mod digest;
mod paths;
pub use digest::PayloadHasher;
pub use paths::relative_path;
use serde::Deserialize;
use std::collections::BTreeSet;

/// Package inventory schema understood by this engine.
pub const SCHEMA_VERSION: u32 = 1;

/// Maximum encoded inventory bytes before decoding untrusted JSON.
pub const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
/// Maximum total expanded bytes across the entire distribution.
pub const MAX_EXPANDED_BYTES: u64 = 16 * 1024 * 1024 * 1024;

/// A package violates the portable distribution contract.
#[derive(Debug, Eq, PartialEq)]
pub enum Rejection {
    /// Unsupported schema or malformed JSON.
    Format,
    /// Encoded inventory, count or expanded bytes exceed a fixed bound.
    Bound,
    /// Unsafe or platform-ambiguous relative name.
    Path,
    /// Duplicate names, invalid hashes, unsupported areas or release identity.
    Inventory,
}
impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Native package rejected: {self:?}")
    }
}
impl std::error::Error for Rejection {}

/// Structurally validated inventory shared by installer extraction and downloaded updates.
pub struct Manifest {
    release_id: String,
    files: Vec<Entry>,
}
/// One exact relative file with declared byte count and digest.
pub struct Entry {
    path: String,
    bytes: u64,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireManifest {
    schema_version: u32,
    release_id: String,
    files: Vec<WireEntry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireEntry {
    path: String,
    bytes: u64,
    sha256: String,
}
impl Manifest {
    /// Decode bounded version-one inventory without performing I/O or granting authenticity.
    /// # Errors
    /// Rejects aliases, path traversal, duplicate entries, malformed values and expansion bombs.
    pub fn decode(bytes: &[u8]) -> Result<Self, Rejection> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(Rejection::Bound);
        }
        let manifest: WireManifest =
            serde_json::from_slice(bytes).map_err(|_| Rejection::Format)?;
        if manifest.schema_version != SCHEMA_VERSION {
            return Err(Rejection::Format);
        }
        if manifest.files.is_empty() || manifest.files.len() > 4096 {
            return Err(Rejection::Bound);
        }
        if manifest.release_id.is_empty()
            || manifest.release_id.len() > 64
            || !manifest.release_id.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
        {
            return Err(Rejection::Inventory);
        }
        let mut names = BTreeSet::new();
        let mut total = 0u64;
        for entry in &manifest.files {
            relative_path(&entry.path)?;
            total = total.checked_add(entry.bytes).ok_or(Rejection::Bound)?;
            if entry.bytes > 2 * 1024 * 1024 * 1024 || total > MAX_EXPANDED_BYTES {
                return Err(Rejection::Bound);
            }
            if !names.insert(entry.path.to_ascii_lowercase())
                || !["app/", "worker/", "electron/", "sugarcubes/"]
                    .iter()
                    .any(|prefix| entry.path.starts_with(prefix))
                || entry.sha256.len() != 64
                || !entry.sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(Rejection::Inventory);
            }
        }
        for name in &names {
            for (offset, _) in name.match_indices('/') {
                if names.contains(&name[..offset]) {
                    return Err(Rejection::Inventory);
                }
            }
        }
        Ok(Self {
            release_id: manifest.release_id,
            files: manifest
                .files
                .into_iter()
                .map(|entry| Entry { path: entry.path, bytes: entry.bytes, sha256: entry.sha256 })
                .collect(),
        })
    }
    /// Release label carried by the authenticated inventory.
    pub fn release(&self) -> &str {
        &self.release_id
    }
    /// Immutable extraction order; concatenated archive bytes follow this exact sequence.
    pub fn entries(&self) -> &[Entry] {
        &self.files
    }
    /// Case-normalized inventory for checking undeclared filesystem content.
    pub fn names(&self) -> BTreeSet<String> {
        self.files.iter().map(|entry| entry.path.to_ascii_lowercase()).collect()
    }
}
impl Entry {
    /// Portable relative name beneath one distribution area.
    pub fn path(&self) -> &str {
        &self.path
    }
    /// Exact expanded byte count.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
    /// Lowercase SHA-256 expected for these file bytes.
    pub fn digest(&self) -> &str {
        &self.sha256
    }
}

#[cfg(test)]
mod tests;
