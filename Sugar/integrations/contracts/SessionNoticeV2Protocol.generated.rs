// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from session-notice-v2.schema.json by scripts/integrations/generate-session-notice-contract.ts.
// Schema SHA-256 (LF): 7efe1bd5b3096ecce2ed4b72219baf7a8c49655ea8716d7419cfdeba70477725

/// Schema-defined untrusted SessionNoticeTone token; decoding admits only the listed values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SessionNoticeTone {
    Info,
    Warn,
    Error,
}

/// Schema-defined untrusted SessionNoticeNotSavedMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct SessionNoticeNotSavedMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) notice: u64,
    pub(super) workspace: bool,
    pub(super) inputs: bool,
}
impl SessionNoticeNotSavedMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.session-notice"
            && self.version == 2
            && valid_panel_session(&self.session)
            && self.r#type == "not-saved"
            && (1..=9007199254740991).contains(&self.notice)
    }
}

/// Schema-defined untrusted SessionNoticeLineMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct SessionNoticeLineMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) notice: u64,
    pub(super) tone: SessionNoticeTone,
    pub(super) text: String,
    pub(super) lasting: bool,
}
impl SessionNoticeLineMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.session-notice"
            && self.version == 2
            && valid_panel_session(&self.session)
            && self.r#type == "line"
            && (1..=9007199254740991).contains(&self.notice)
            && self.text.chars().count() <= 400
    }
}

/// Schema-defined untrusted SessionNoticeNoneMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct SessionNoticeNoneMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
}
impl SessionNoticeNoneMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.session-notice"
            && self.version == 2
            && valid_panel_session(&self.session)
            && self.r#type == "none"
    }
}

/// Schema-defined untrusted SessionNoticeWatchMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct SessionNoticeWatchMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
}
impl SessionNoticeWatchMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.session-notice"
            && self.version == 2
            && valid_panel_session(&self.session)
            && self.r#type == "watch"
    }
}

/// Schema-defined untrusted SessionNoticeShownMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct SessionNoticeShownMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) notice: u64,
}
impl SessionNoticeShownMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.session-notice"
            && self.version == 2
            && valid_panel_session(&self.session)
            && self.r#type == "shown"
            && (1..=9007199254740991).contains(&self.notice)
    }
}

/// Schema-defined untrusted SessionNoticeHostMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum SessionNoticeHostMessage {
    NotSaved(SessionNoticeNotSavedMessage),
    Line(SessionNoticeLineMessage),
    None(SessionNoticeNoneMessage),
}
impl SessionNoticeHostMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::NotSaved(value) => value.valid(),
            Self::Line(value) => value.valid(),
            Self::None(value) => value.valid(),
        }
    }
}

/// Schema-defined untrusted SessionNoticeClientMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum SessionNoticeClientMessage {
    Watch(SessionNoticeWatchMessage),
    Shown(SessionNoticeShownMessage),
}
impl SessionNoticeClientMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Watch(value) => value.valid(),
            Self::Shown(value) => value.valid(),
        }
    }
}

pub(super) fn valid_panel_session(value: &str) -> bool {
    (16..=80).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}
