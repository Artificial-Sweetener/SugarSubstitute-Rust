// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from file-choice.schema.json by scripts/integrations/generate-file-choice-contract.ts.
// Schema SHA-256 (LF): 94bab1f563c262bdc0a2daad99f1fefed989cfde22977894862b91d96a05ef9e

/// Schema-defined untrusted FileChoiceFailure token; decoding admits only the listed values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum FileChoiceFailure {
    Busy,
    Unavailable,
}

/// Schema-defined untrusted FileChoiceChosenMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct FileChoiceChosenMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) request: String,
    pub(super) name: String,
    pub(super) relative: String,
}
impl FileChoiceChosenMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.file-choice"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "chosen"
            && valid_lowercase_token(&self.request, 48)
            && self.name.encode_utf16().count() <= 255
            && self.relative.encode_utf16().count() <= 1024
    }
}

/// Schema-defined untrusted FileChoiceDismissedMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct FileChoiceDismissedMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) request: String,
}
impl FileChoiceDismissedMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.file-choice"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "dismissed"
            && valid_lowercase_token(&self.request, 48)
    }
}

/// Schema-defined untrusted FileChoiceFailedMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct FileChoiceFailedMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) request: String,
    pub(super) reason: FileChoiceFailure,
}
impl FileChoiceFailedMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.file-choice"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "failed"
            && valid_lowercase_token(&self.request, 48)
    }
}

/// Schema-defined untrusted FileChoiceOpenMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct FileChoiceOpenMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) request: String,
    pub(super) folder: String,
    pub(super) extension: String,
}
impl FileChoiceOpenMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.file-choice"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "open"
            && valid_lowercase_token(&self.request, 48)
            && self.folder.encode_utf16().count() <= 1024
            && valid_lowercase_token(&self.extension, 64)
    }
}

/// Schema-defined untrusted FileChoiceSaveMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct FileChoiceSaveMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) request: String,
    pub(super) folder: String,
    pub(super) name: String,
    pub(super) extension: String,
}
impl FileChoiceSaveMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.file-choice"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "save"
            && valid_lowercase_token(&self.request, 48)
            && self.folder.encode_utf16().count() <= 1024
            && self.name.encode_utf16().count() <= 255
            && valid_lowercase_token(&self.extension, 64)
    }
}

/// Schema-defined untrusted FileChoiceHostMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum FileChoiceHostMessage {
    Chosen(FileChoiceChosenMessage),
    Dismissed(FileChoiceDismissedMessage),
    Failed(FileChoiceFailedMessage),
}
impl FileChoiceHostMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Chosen(value) => value.valid(),
            Self::Dismissed(value) => value.valid(),
            Self::Failed(value) => value.valid(),
        }
    }
}

/// Schema-defined untrusted FileChoiceClientMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum FileChoiceClientMessage {
    Open(FileChoiceOpenMessage),
    Save(FileChoiceSaveMessage),
}
impl FileChoiceClientMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Open(value) => value.valid(),
            Self::Save(value) => value.valid(),
        }
    }
}

pub(super) fn valid_panel_session(value: &str) -> bool {
    (16..=80).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_lowercase_token(value: &str, maximum: usize) -> bool {
    (1..=maximum).contains(&value.len())
        && value.bytes().all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-'))
}
