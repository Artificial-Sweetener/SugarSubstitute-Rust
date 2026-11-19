// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from update-report.schema.json by scripts/integrations/generate-update-report-contract.ts.
// Schema SHA-256 (LF): 55c624a3b31e84dbe4493d8565f22d7755399b2c4c368a4c3889ac1099fda224

/// Schema-defined untrusted UpdateReportStateMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct UpdateReportStateMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) revision: u64,
    pub(super) available: bool,
}
impl UpdateReportStateMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.update-report"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "state"
            && (0..=9007199254740991).contains(&self.revision)
    }
}

/// Schema-defined untrusted UpdateReportWatchMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct UpdateReportWatchMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
}
impl UpdateReportWatchMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.update-report"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "watch"
    }
}

/// Schema-defined untrusted UpdateReportOpenMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct UpdateReportOpenMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) revision: u64,
}
impl UpdateReportOpenMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.update-report"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "open"
            && (0..=9007199254740991).contains(&self.revision)
    }
}

/// Schema-defined untrusted UpdateReportClientMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum UpdateReportClientMessage {
    Watch(UpdateReportWatchMessage),
    Open(UpdateReportOpenMessage),
}
impl UpdateReportClientMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Watch(value) => value.valid(),
            Self::Open(value) => value.valid(),
        }
    }
}

pub(super) fn valid_panel_session(value: &str) -> bool {
    (16..=80).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}
