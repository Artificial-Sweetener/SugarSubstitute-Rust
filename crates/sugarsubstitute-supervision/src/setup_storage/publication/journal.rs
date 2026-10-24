// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind cleanup authority to an exact parent, opaque attempt name and native file identity.
use super::*;
use serde::{Deserialize, Serialize};
const PREFIX: &str = ".substitute-publication-";

/// Complete deletion authority for one unpublished sibling under its original parent.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    schema_version: u32,
    id: String,
    parent: (u32, u32, u32),
    pub(super) file: (u32, u32, u32),
}
impl Record {
    /// Opaque correlation identity for bounded cleanup diagnostics.
    pub(super) fn id(&self) -> &str {
        &self.id
    }
    /// Record only identities obtained from the creator's retained native handles.
    pub(super) fn new(id: String, parent: (u32, u32, u32), file: (u32, u32, u32)) -> Self {
        Self { schema_version: 1, id, parent, file }
    }
    /// Recognize a complete canonical journal leaf, without treating its name as ownership.
    pub(super) fn journal_id(name: &str) -> Option<&str> {
        let id = name.strip_prefix(PREFIX)?.strip_suffix(".json")?;
        (id.len() == 32 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
            .then_some(id)
    }
    /// No record can direct cleanup outside its exact, purpose-specific sibling stage name.
    pub(super) fn stage_name(id: &str) -> String {
        format!("{PREFIX}{id}.stage")
    }
    /// Return the one stage leaf determined by this verified attempt identity.
    pub(super) fn stage(&self) -> String {
        Self::stage_name(&self.id)
    }
    /// Return the ownership record's canonical name in the same retained parent.
    pub(super) fn journal_name(&self) -> String {
        format!("{PREFIX}{}.json", self.id)
    }
    /// Reject unknown schemas, cross-directory moves and mismatched attempt identities.
    pub(super) fn decode(bytes: &[u8], id: &str, parent: (u32, u32, u32)) -> io::Result<Self> {
        let record: Self = serde_json::from_slice(bytes).map_err(io::Error::other)?;
        if record.schema_version != 1
            || record.id != id
            || record.parent != parent
            || record.file == (0, 0, 0)
        {
            return Err(invalid("Publication journal ownership is unsupported or changed"));
        }
        Ok(record)
    }
}
