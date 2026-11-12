// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from updates.schema.json by scripts/integrations/generate-updates-contract.ts.
// Schema SHA-256 (LF): daa78a25b7c91de8961c770ed9187dee8677bfd244081d919aa1b29a8bbb85d2

/// Schema-defined untrusted UpdatesChannel token; decoding admits only the listed values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum UpdatesChannel {
    Stable,
    Beta,
}

/// Schema-defined untrusted UpdatesFailure token; decoding admits only the listed values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum UpdatesFailure {
    Check,
    Update,
}

/// Schema-defined untrusted UpdatesPhase token; decoding admits only the listed values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum UpdatesPhase {
    Check,
    Download,
    Verify,
    Install,
    Start,
}

/// Schema-defined untrusted UpdatesScheduledRestart token; decoding admits only the listed values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum UpdatesScheduledRestart {
    None,
    AfterWorkflow,
}

/// Schema-defined untrusted UpdatesRestartTiming token; decoding admits only the listed values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum UpdatesRestartTiming {
    Now,
    AfterWorkflow,
}

/// Schema-defined untrusted UpdatesRejection token; decoding admits only the listed values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum UpdatesRejection {
    Busy,
    NotReady,
    Unavailable,
}

/// Schema-defined untrusted UpdatesCheck data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct UpdatesCheck {
    pub(super) completed_at: u64,
}
impl UpdatesCheck {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (0..=9007199254740991).contains(&self.completed_at)
    }
}

/// Schema-defined untrusted UpdatesStatus data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "state", deny_unknown_fields, rename_all_fields = "camelCase")]
pub(super) enum UpdatesStatus {
    #[serde(rename = "up-to-date")]
    UpToDate {},
    #[serde(rename = "checking")]
    Checking {},
    #[serde(rename = "available")]
    Available { version: String },
    #[serde(rename = "ready")]
    Ready { version: String },
    #[serde(rename = "failed")]
    Failed { during: UpdatesFailure },
}
impl UpdatesStatus {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::UpToDate {} => true,
            Self::Checking {} => true,
            Self::Available { version } => valid_lowercase_token(version, 64),
            Self::Ready { version } => valid_lowercase_token(version, 64),
            Self::Failed { during: _ } => true,
        }
    }
}

/// Schema-defined untrusted UpdatesCause data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct UpdatesCause {
    pub(super) message: String,
}
impl UpdatesCause {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.message.chars().count() <= 1024
    }
}

/// Schema-defined untrusted UpdatesReport data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct UpdatesReport {
    pub(super) version: String,
    pub(super) phase: UpdatesPhase,
    pub(super) causes: Vec<UpdatesCause>,
}
impl UpdatesReport {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        valid_lowercase_token(&self.version, 64)
            && !self.causes.is_empty()
            && self.causes.len() <= 8
            && self.causes.iter().all(UpdatesCause::valid)
    }
}

/// Schema-defined untrusted UpdatesState data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct UpdatesState {
    pub(super) current_version: String,
    pub(super) channel: UpdatesChannel,
    #[serde(
        default,
        deserialize_with = "deserialize_present_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) last_successful_check: Option<UpdatesCheck>,
    pub(super) status: UpdatesStatus,
    #[serde(
        default,
        deserialize_with = "deserialize_present_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) report: Option<UpdatesReport>,
    pub(super) previous_version_running: bool,
    pub(super) workflow_running: bool,
    pub(super) scheduled_restart: UpdatesScheduledRestart,
}
impl UpdatesState {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        valid_lowercase_token(&self.current_version, 64)
            && self.last_successful_check.as_ref().is_none_or(UpdatesCheck::valid)
            && self.status.valid()
            && self.report.as_ref().is_none_or(UpdatesReport::valid)
    }
}

/// Schema-defined untrusted UpdatesStateMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct UpdatesStateMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) revision: u64,
    pub(super) update: UpdatesState,
}
impl UpdatesStateMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.updates"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "state"
            && (1..=9007199254740991).contains(&self.revision)
            && self.update.valid()
    }
}

/// Schema-defined untrusted UpdatesRejectedMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct UpdatesRejectedMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) request: String,
    pub(super) reason: UpdatesRejection,
}
impl UpdatesRejectedMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.updates"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "rejected"
            && valid_lowercase_token(&self.request, 48)
    }
}

/// Schema-defined untrusted UpdatesAcceptedMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct UpdatesAcceptedMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) request: String,
}
impl UpdatesAcceptedMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.updates"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "accepted"
            && valid_lowercase_token(&self.request, 48)
    }
}

/// Schema-defined untrusted UpdatesWatchMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct UpdatesWatchMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
}
impl UpdatesWatchMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.updates"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "watch"
    }
}

/// Schema-defined untrusted UpdatesCheckMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct UpdatesCheckMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) request: String,
}
impl UpdatesCheckMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.updates"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "check"
            && valid_lowercase_token(&self.request, 48)
    }
}

/// Schema-defined untrusted UpdatesRestartMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct UpdatesRestartMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) request: String,
    pub(super) when: UpdatesRestartTiming,
}
impl UpdatesRestartMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.updates"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "restart"
            && valid_lowercase_token(&self.request, 48)
    }
}

/// Schema-defined untrusted UpdatesHostMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum UpdatesHostMessage {
    State(UpdatesStateMessage),
    Accepted(UpdatesAcceptedMessage),
    Rejected(UpdatesRejectedMessage),
}
impl UpdatesHostMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::State(value) => value.valid(),
            Self::Accepted(value) => value.valid(),
            Self::Rejected(value) => value.valid(),
        }
    }
}

/// Schema-defined untrusted UpdatesClientMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum UpdatesClientMessage {
    Watch(UpdatesWatchMessage),
    Check(UpdatesCheckMessage),
    Restart(UpdatesRestartMessage),
}
impl UpdatesClientMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Watch(value) => value.valid(),
            Self::Check(value) => value.valid(),
            Self::Restart(value) => value.valid(),
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

/// Decode an optional member only when present, rejecting an explicit null.
fn deserialize_present_option<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}
