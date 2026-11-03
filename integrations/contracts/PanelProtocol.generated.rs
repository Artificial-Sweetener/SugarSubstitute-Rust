// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from panel.schema.json by scripts/integrations/generate-panel-contract.ts.
// Schema SHA-256 (LF): 166ad0c95a5dda530f712ce520d4bd52823a07a2555d3006a922a8444cebb16e

/// Schema-defined untrusted PanelPresentation data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct PanelPresentation {
    pub(super) plugin_mode: bool,
    pub(super) visual_style: String,
    pub(super) photo_pro_palette: String,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub(super) accent_color: Option<String>,
    pub(super) resize_corner_size: u64,
}
impl PanelPresentation {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        valid_panel_presentation_visual_style(&self.visual_style)
            && valid_panel_presentation_photo_pro_palette(&self.photo_pro_palette)
            && valid_optional_color(self.accent_color.as_deref())
            && (0..=64).contains(&self.resize_corner_size)
    }
}

/// Schema-defined untrusted PanelReady data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct PanelReady {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
}
impl PanelReady {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.panel"
            && self.version == 3
            && valid_panel_session(&self.session)
            && self.r#type == "ready"
    }
}

/// Schema-defined untrusted PanelConfigure data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct PanelConfigure {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) presentation: PanelPresentation,
}
impl PanelConfigure {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.panel"
            && self.version == 3
            && valid_panel_session(&self.session)
            && self.r#type == "configure"
            && self.presentation.valid()
    }
}

/// Schema-defined untrusted PanelApplied data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct PanelApplied {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
}
impl PanelApplied {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.panel"
            && self.version == 3
            && valid_panel_session(&self.session)
            && self.r#type == "applied"
    }
}

/// Schema-defined untrusted PanelPing data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct PanelPing {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
}
impl PanelPing {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.panel"
            && self.version == 3
            && valid_panel_session(&self.session)
            && self.r#type == "ping"
    }
}

/// Schema-defined untrusted PanelPong data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct PanelPong {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) connected: bool,
}
impl PanelPong {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.panel"
            && self.version == 3
            && valid_panel_session(&self.session)
            && self.r#type == "pong"
    }
}

/// Schema-defined untrusted PanelRejected data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct PanelRejected {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) r#type: String,
    pub(super) reason: String,
}
impl PanelRejected {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.panel"
            && self.version == 3
            && valid_panel_session(&self.session)
            && self.r#type == "rejected"
            && valid_panel_rejected_reason(&self.reason)
    }
}

fn valid_panel_presentation_visual_style(value: &str) -> bool {
    matches!(value, "comfy" | "fluent" | "spectrum" | "photopro" | "gimp")
}

fn valid_panel_presentation_photo_pro_palette(value: &str) -> bool {
    matches!(value, "black" | "dark-gray" | "medium-gray" | "light-gray")
}

pub(super) fn valid_optional_color(value: Option<&str>) -> bool {
    value.is_none_or(|value| {
        value.len() == 7
            && value.starts_with('#')
            && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
    })
}

pub(super) fn valid_panel_session(value: &str) -> bool {
    (16..=80).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_panel_rejected_reason(value: &str) -> bool {
    matches!(value, "unsupported-presentation" | "presentation-failed")
}

/// Preserve a required nullable field instead of defaulting a missing member to null.
fn deserialize_required_option<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    serde::Deserialize::deserialize(deserializer)
}
