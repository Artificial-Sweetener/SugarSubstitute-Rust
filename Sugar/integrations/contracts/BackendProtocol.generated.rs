// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from backend.schema.json by scripts/integrations/generate-backend-contract.ts.
// Schema SHA-256 (LF): 9ba68e6284f1e7bedfcb1dd2fbf8c54456c998e3f4895e51e556748009219a0f

/// Schema-defined untrusted BackendStatus token; decoding admits only the listed values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum BackendStatus {
    Running,
    Stopped,
    RetryReady,
    Retrying,
}

/// Schema-defined untrusted BackendStateMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct BackendStateMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) revision: u64,
    pub(super) status: BackendStatus,
}
impl BackendStateMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.backend"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "state"
            && (0..=9007199254740991).contains(&self.revision)
    }
}

/// Schema-defined untrusted BackendWatchMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct BackendWatchMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
}
impl BackendWatchMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.backend"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "watch"
    }
}

/// Schema-defined untrusted BackendRetryMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct BackendRetryMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) revision: u64,
}
impl BackendRetryMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.backend"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "retry"
            && (1..=9007199254740991).contains(&self.revision)
    }
}

/// Schema-defined untrusted BackendClientMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum BackendClientMessage {
    Watch(BackendWatchMessage),
    Retry(BackendRetryMessage),
}
impl BackendClientMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Watch(value) => value.valid(),
            Self::Retry(value) => value.valid(),
        }
    }
}

pub(super) fn valid_panel_session(value: &str) -> bool {
    (16..=80).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}
