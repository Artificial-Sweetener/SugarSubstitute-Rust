// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from file-drag.schema.json by scripts/integrations/generate-file-drag-contract.ts.
// Schema SHA-256 (LF): 393166aaff46aadfb8c74b7b492ae25425cb8eea91c7f490f5ae2431e1b1fade

/// Schema-defined untrusted FileDragFile data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct FileDragFile {
    pub(super) name: String,
}
impl FileDragFile {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.name.encode_utf16().count() <= 255
    }
}

/// Schema-defined untrusted FileDragEnteredMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct FileDragEnteredMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) files: Vec<FileDragFile>,
}
impl FileDragEnteredMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.file-drag"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "entered"
            && !self.files.is_empty()
            && self.files.len() <= 16
            && self.files.iter().all(FileDragFile::valid)
    }
}

/// Schema-defined untrusted FileDragLeftMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct FileDragLeftMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
}
impl FileDragLeftMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.file-drag"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "left"
    }
}

/// Schema-defined untrusted FileDragHostMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum FileDragHostMessage {
    Entered(FileDragEnteredMessage),
    Left(FileDragLeftMessage),
}
impl FileDragHostMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Entered(value) => value.valid(),
            Self::Left(value) => value.valid(),
        }
    }
}

pub(super) fn valid_panel_session(value: &str) -> bool {
    (16..=80).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}
