// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from release-notes.schema.json by scripts/integrations/generate-release-notes-contract.ts.
// Schema SHA-256 (LF): 9c5a20d855c5063bab38d4d0817e14a50bf86bed44f02d05267966c94bf8f0d6

/// Schema-defined untrusted ReleaseNotesFailure token; decoding admits only the listed values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ReleaseNotesFailure {
    Unavailable,
    NotPublished,
}

/// Schema-defined untrusted ReleaseNotesRelease data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ReleaseNotesRelease {
    pub(super) version: String,
    pub(super) notes: String,
}
impl ReleaseNotesRelease {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        valid_lowercase_token(&self.version, 64) && self.notes.encode_utf16().count() <= 262144
    }
}

/// Schema-defined untrusted ReleaseNotesStateMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ReleaseNotesStateMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) revision: u64,
    pub(super) product: String,
    pub(super) current_version: String,
}
impl ReleaseNotesStateMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.release-notes"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "state"
            && (1..=9007199254740991).contains(&self.revision)
            && self.product.encode_utf16().count() <= 64
            && valid_lowercase_token(&self.current_version, 64)
    }
}

/// Schema-defined untrusted ReleaseNotesOpenMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ReleaseNotesOpenMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
}
impl ReleaseNotesOpenMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.release-notes"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "open"
    }
}

/// Schema-defined untrusted ReleaseNotesNotesMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ReleaseNotesNotesMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) request: String,
    pub(super) releases: Vec<ReleaseNotesRelease>,
    pub(super) omitted: u64,
}
impl ReleaseNotesNotesMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.release-notes"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "notes"
            && valid_lowercase_token(&self.request, 48)
            && !self.releases.is_empty()
            && self.releases.len() <= 64
            && self.releases.iter().all(ReleaseNotesRelease::valid)
            && (0..=9007199254740991).contains(&self.omitted)
    }
}

/// Schema-defined untrusted ReleaseNotesFailedMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ReleaseNotesFailedMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) request: String,
    pub(super) reason: ReleaseNotesFailure,
}
impl ReleaseNotesFailedMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.release-notes"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "failed"
            && valid_lowercase_token(&self.request, 48)
    }
}

/// Schema-defined untrusted ReleaseNotesWatchMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ReleaseNotesWatchMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
}
impl ReleaseNotesWatchMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.release-notes"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "watch"
    }
}

/// Schema-defined untrusted ReleaseNotesLoadMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ReleaseNotesLoadMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) request: String,
    pub(super) since: String,
}
impl ReleaseNotesLoadMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.release-notes"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "load"
            && valid_lowercase_token(&self.request, 48)
            && valid_lowercase_token(&self.since, 64)
    }
}

/// Schema-defined untrusted ReleaseNotesHostMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum ReleaseNotesHostMessage {
    State(ReleaseNotesStateMessage),
    Open(ReleaseNotesOpenMessage),
    Notes(ReleaseNotesNotesMessage),
    Failed(ReleaseNotesFailedMessage),
}
impl ReleaseNotesHostMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::State(value) => value.valid(),
            Self::Open(value) => value.valid(),
            Self::Notes(value) => value.valid(),
            Self::Failed(value) => value.valid(),
        }
    }
}

/// Schema-defined untrusted ReleaseNotesClientMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum ReleaseNotesClientMessage {
    Watch(ReleaseNotesWatchMessage),
    Load(ReleaseNotesLoadMessage),
}
impl ReleaseNotesClientMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Watch(value) => value.valid(),
            Self::Load(value) => value.valid(),
        }
    }
}

fn valid_lowercase_token(value: &str, maximum: usize) -> bool {
    (1..=maximum).contains(&value.len())
        && value.bytes().all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-'))
}

pub(super) fn valid_panel_session(value: &str) -> bool {
    (16..=80).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}
