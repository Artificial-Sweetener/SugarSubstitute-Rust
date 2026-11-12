// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from memory.schema.json by scripts/integrations/generate-memory-contract.ts.
// Schema SHA-256 (LF): 8bc99fed406e1fe869b8275e6b06df543c45b9f05ec4248b5c2198a1c277ab74

/// Schema-defined untrusted MemoryLevel token; decoding admits only the listed values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum MemoryLevel {
    Normal,
    Elevated,
    High,
    Critical,
}

/// Schema-defined untrusted MemoryPreference data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "mode", deny_unknown_fields, rename_all_fields = "camelCase")]
pub(super) enum MemoryPreference {
    #[serde(rename = "share")]
    Share { percent: u64 },
    #[serde(rename = "fixed")]
    Fixed { bytes: u64 },
}
impl MemoryPreference {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Share { percent } => (10..=75).contains(percent),
            Self::Fixed { bytes } => (1..=9007199254740991).contains(bytes),
        }
    }
}

/// Schema-defined untrusted MemoryConsumer data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct MemoryConsumer {
    pub(super) id: String,
    pub(super) bytes: u64,
}
impl MemoryConsumer {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        valid_lowercase_token(&self.id, 48) && (0..=9007199254740991).contains(&self.bytes)
    }
}

/// Schema-defined untrusted MemoryUsageMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct MemoryUsageMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) used_bytes: u64,
    pub(super) demand_bytes: u64,
    pub(super) consumers: Vec<MemoryConsumer>,
}
impl MemoryUsageMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.memory"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "usage"
            && (0..=9007199254740991).contains(&self.used_bytes)
            && (0..=9007199254740991).contains(&self.demand_bytes)
            && self.consumers.len() <= 16
            && self.consumers.iter().all(MemoryConsumer::valid)
    }
}

/// Schema-defined untrusted MemoryPreferenceMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct MemoryPreferenceMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) preference: MemoryPreference,
}
impl MemoryPreferenceMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.memory"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "preference"
            && self.preference.valid()
    }
}

/// Schema-defined untrusted MemoryAllowanceMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct MemoryAllowanceMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) generation: u64,
    pub(super) level: MemoryLevel,
    pub(super) allowance_bytes: u64,
    pub(super) ceiling_bytes: u64,
    pub(super) total_bytes: u64,
    pub(super) preference: MemoryPreference,
}
impl MemoryAllowanceMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.memory"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "allowance"
            && (0..=9007199254740991).contains(&self.generation)
            && (0..=9007199254740991).contains(&self.allowance_bytes)
            && (0..=9007199254740991).contains(&self.ceiling_bytes)
            && (0..=9007199254740991).contains(&self.total_bytes)
            && self.preference.valid()
    }
}

/// Schema-defined untrusted MemoryClientMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum MemoryClientMessage {
    Usage(MemoryUsageMessage),
    Preference(MemoryPreferenceMessage),
}
impl MemoryClientMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Usage(value) => value.valid(),
            Self::Preference(value) => value.valid(),
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
