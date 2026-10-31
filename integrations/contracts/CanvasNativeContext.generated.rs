// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from canvas.schema.json by scripts/integrations/generate-canvas-contract.ts.
// Schema SHA-256 (LF): d5a712b0e68d1df579761a5a224bcbc3a6a62c70163ae418c849b3efaa225159

/// Schema-defined untrusted CanvasReadRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasReadRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
}
impl CanvasReadRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "context.request"
    }
}

/// Schema-defined untrusted CanvasFailure data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasFailure {
    pub(super) status: String,
    pub(super) message: String,
}
impl CanvasFailure {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        valid_canvas_failure_status(&self.status) && self.message.encode_utf16().count() <= 512
    }
}

/// Schema-defined untrusted CanvasErrorReply data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasErrorReply {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) failure: CanvasFailure,
}
impl CanvasErrorReply {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "error.reply"
            && self.failure.valid()
    }
}

/// Schema-defined untrusted CanvasNativeCapabilities data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasNativeCapabilities {
    pub(super) display_output: bool,
    pub(super) insert_image_layer: bool,
    pub(super) position_image_layer: bool,
}
impl CanvasNativeCapabilities {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        true
    }
}

/// Schema-defined untrusted CanvasNativeDescriptor data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasNativeDescriptor {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) capabilities: CanvasNativeCapabilities,
}
impl CanvasNativeDescriptor {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.id.encode_utf16().count() <= 128
            && self.label.encode_utf16().count() <= 128
            && self.capabilities.valid()
    }
}

/// Schema-defined untrusted CanvasNativeContext data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasNativeContext {
    pub(super) canvas: CanvasNativeDescriptor,
    pub(super) document: (),
}
impl CanvasNativeContext {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.canvas.valid()
    }
}

/// Schema-defined untrusted CanvasNativeReadReply data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasNativeReadReply {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) context: CanvasNativeContext,
}
impl CanvasNativeReadReply {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "context.reply"
            && self.context.valid()
    }
}

pub(super) fn valid_panel_session(value: &str) -> bool {
    (16..=80).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_canvas_failure_status(value: &str) -> bool {
    matches!(value, "unavailable" | "unsupported" | "stale" | "cancelled" | "error")
}
