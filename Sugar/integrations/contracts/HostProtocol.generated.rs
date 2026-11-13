// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from host.schema.json by scripts/integrations/generate-host-contract.ts.
// Schema SHA-256 (LF): 4903b48224a09b12c1832bc4feb6f5eb4580dbc99445a92dc37bdb3f0025864d

/// Schema-defined untrusted HostCapability data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct HostCapability {
    pub(super) name: String,
}
impl HostCapability {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        valid_lowercase_token(&self.name, 64)
    }
}

/// Schema-defined untrusted HostProtocol data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct HostProtocol {
    pub(super) name: String,
    pub(super) version: u64,
}
impl HostProtocol {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        valid_lowercase_token(&self.name, 64) && (1..=65535).contains(&self.version)
    }
}

/// Schema-defined untrusted HostDescriptionMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct HostDescriptionMessage {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) protocols: Vec<HostProtocol>,
    pub(super) capabilities: Vec<HostCapability>,
}
impl HostDescriptionMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.host"
            && self.version == 1
            && valid_panel_session(&self.session)
            && self.r#type == "description"
            && self.protocols.len() <= 32
            && self.protocols.iter().all(HostProtocol::valid)
            && self.capabilities.len() <= 64
            && self.capabilities.iter().all(HostCapability::valid)
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
