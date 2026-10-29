// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from canvas.schema.json by scripts/integrations/generate-canvas-contract.ts.
// Schema SHA-256 (LF): 3931c2aaf3b10ef6548c48624085c0a314aaf3e8e00acb7341428b524472cc57

/// Schema-defined untrusted CanvasSurfaceCapabilities data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceCapabilities {
    pub(super) schema: u64,
    pub(super) image_formats: Vec<String>,
    pub(super) viewport_commands: Vec<String>,
    pub(super) viewport_state: bool,
    pub(super) preview_formats: Vec<String>,
    pub(super) comparison: bool,
    pub(super) overview: bool,
}
impl CanvasSurfaceCapabilities {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.schema == 1
            && valid_canvas_surface_capabilities_image_formats(&self.image_formats)
            && valid_canvas_surface_capabilities_viewport_commands(&self.viewport_commands)
            && valid_canvas_surface_capabilities_preview_formats(&self.preview_formats)
    }
}

/// Schema-defined untrusted CanvasSurfaceCapabilitiesRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceCapabilitiesRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) schema: u64,
}
impl CanvasSurfaceCapabilitiesRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.capabilities.request"
            && self.schema == 1
    }
}

/// Schema-defined untrusted CanvasSurfaceCapabilitiesReply data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceCapabilitiesReply {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) status: String,
    pub(super) capabilities: CanvasSurfaceCapabilities,
    pub(super) message: String,
}
impl CanvasSurfaceCapabilitiesReply {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.capabilities.reply"
            && valid_canvas_surface_capabilities_reply_status(&self.status)
            && self.capabilities.valid()
            && self.message.chars().count() <= 512
    }
}

/// Schema-defined untrusted CanvasSurfaceAddress data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceAddress {
    pub(super) session_id: String,
    pub(super) view_id: String,
    pub(super) window_id: String,
    pub(super) attachment_id: String,
    pub(super) generation: u64,
}
impl CanvasSurfaceAddress {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        valid_surface_identity(&self.session_id, "canvas:", 256)
            && valid_surface_identity(&self.view_id, "view:", 128)
            && valid_surface_identity(&self.window_id, "window:", 128)
            && valid_surface_identity(&self.attachment_id, "attachment:", 128)
            && (1..=9007199254740991).contains(&self.generation)
    }
}

/// Schema-defined untrusted CanvasSurfaceFileReference data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceFileReference {
    pub(super) filename: String,
    pub(super) subfolder: String,
    pub(super) r#type: String,
}
impl CanvasSurfaceFileReference {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.filename.chars().count() <= 4096
            && self.subfolder.chars().count() <= 4096
            && matches!(self.r#type.as_str(), "input" | "output" | "temp")
    }
}

/// Schema-defined untrusted CanvasSurfacePoint data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfacePoint {
    pub(super) x: f64,
    pub(super) y: f64,
}
impl CanvasSurfacePoint {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (-1000000.0..=1000000.0).contains(&self.x) && (-1000000.0..=1000000.0).contains(&self.y)
    }
}

/// Schema-defined untrusted CanvasSurfaceSize data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceSize {
    pub(super) width: f64,
    pub(super) height: f64,
}
impl CanvasSurfaceSize {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (0.0..=1000000.0).contains(&self.width) && (0.0..=1000000.0).contains(&self.height)
    }
}

/// Schema-defined untrusted CanvasSurfaceClip data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceClip {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}
impl CanvasSurfaceClip {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (-1000000.0..=1000000.0).contains(&self.x)
            && (-1000000.0..=1000000.0).contains(&self.y)
            && (0.0..=1000000.0).contains(&self.width)
            && (0.0..=1000000.0).contains(&self.height)
    }
}

/// Schema-defined untrusted CanvasSurfaceGeometry data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceGeometry {
    pub(super) coordinate_space: String,
    pub(super) units: String,
    pub(super) origin: CanvasSurfacePoint,
    pub(super) viewport: CanvasSurfaceSize,
    pub(super) clip: CanvasSurfaceClip,
    pub(super) physical_pixels_per_css_pixel: f64,
    pub(super) visible: bool,
    pub(super) focused: bool,
}
impl CanvasSurfaceGeometry {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.coordinate_space == "client"
            && self.units == "css-pixels"
            && self.origin.valid()
            && self.viewport.valid()
            && self.clip.valid()
            && (0.0..=64.0).contains(&self.physical_pixels_per_css_pixel)
            && self.physical_pixels_per_css_pixel > 0.0
    }
}

/// Schema-defined untrusted CanvasSurfacePrepareRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfacePrepareRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) address: CanvasSurfaceAddress,
    pub(super) revision: u64,
}
impl CanvasSurfacePrepareRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.prepare"
            && self.address.valid()
            && (1..=9007199254740991).contains(&self.revision)
    }
}

/// Schema-defined untrusted CanvasSurfaceGeometryRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceGeometryRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) address: CanvasSurfaceAddress,
    pub(super) revision: u64,
    pub(super) geometry: CanvasSurfaceGeometry,
}
impl CanvasSurfaceGeometryRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.geometry"
            && self.address.valid()
            && (1..=9007199254740991).contains(&self.revision)
            && self.geometry.valid()
    }
}

/// Schema-defined untrusted CanvasSurfaceSelectRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceSelectRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) address: CanvasSurfaceAddress,
    pub(super) revision: u64,
    pub(super) item_id: String,
    pub(super) file: CanvasSurfaceFileReference,
}
impl CanvasSurfaceSelectRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.select"
            && self.address.valid()
            && (1..=9007199254740991).contains(&self.revision)
            && valid_surface_identity(&self.item_id, "result:", 4096)
            && self.file.valid()
    }
}

/// Schema-defined untrusted CanvasSurfaceCommitRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceCommitRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) address: CanvasSurfaceAddress,
    pub(super) revision: u64,
}
impl CanvasSurfaceCommitRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.commit"
            && self.address.valid()
            && (1..=9007199254740991).contains(&self.revision)
    }
}

/// Schema-defined untrusted CanvasSurfaceReleaseRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceReleaseRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) address: CanvasSurfaceAddress,
    pub(super) revision: u64,
}
impl CanvasSurfaceReleaseRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.release"
            && self.address.valid()
            && (1..=9007199254740991).contains(&self.revision)
    }
}

/// Schema-defined untrusted CanvasSurfaceCancelRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceCancelRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
}
impl CanvasSurfaceCancelRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "cancel"
    }
}

/// Schema-defined untrusted CanvasSurfaceReply data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceReply {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) address: CanvasSurfaceAddress,
    pub(super) revision: u64,
    pub(super) status: String,
    pub(super) message: String,
}
impl CanvasSurfaceReply {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.reply"
            && self.address.valid()
            && (1..=9007199254740991).contains(&self.revision)
            && valid_canvas_surface_reply_status(&self.status)
            && self.message.chars().count() <= 512
    }
}

/// Schema-defined untrusted CanvasSurfaceViewportCommand data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", deny_unknown_fields, rename_all_fields = "camelCase")]
pub(super) enum CanvasSurfaceViewportCommand {
    #[serde(rename = "read")]
    Read {},
    #[serde(rename = "fit")]
    Fit {},
    #[serde(rename = "native")]
    Native {},
    #[serde(rename = "zoom")]
    Zoom { factor: f64 },
    #[serde(rename = "pan")]
    Pan { x: f64, y: f64, units: String },
}
impl CanvasSurfaceViewportCommand {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Read {} => true,
            Self::Fit {} => true,
            Self::Native {} => true,
            Self::Zoom { factor } => (0.01..=100.0).contains(factor),
            Self::Pan { x, y, units } => {
                (-10.0..=10.0).contains(x)
                    && (-10.0..=10.0).contains(y)
                    && units.as_str() == "viewport"
            }
        }
    }
}

/// Schema-defined untrusted CanvasSurfaceImageDimensions data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceImageDimensions {
    pub(super) width: u64,
    pub(super) height: u64,
}
impl CanvasSurfaceImageDimensions {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (1..=2147483647).contains(&self.width) && (1..=2147483647).contains(&self.height)
    }
}

/// Schema-defined untrusted CanvasSurfaceViewportState data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceViewportState {
    pub(super) mode: String,
    pub(super) physical_scale: f64,
    pub(super) dimensions: CanvasSurfaceImageDimensions,
}
impl CanvasSurfaceViewportState {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        matches!(self.mode.as_str(), "fit" | "native" | "custom")
            && (1e-9..=16384.0).contains(&self.physical_scale)
            && self.dimensions.valid()
    }
}

/// Schema-defined untrusted CanvasSurfaceViewportRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceViewportRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) address: CanvasSurfaceAddress,
    pub(super) revision: u64,
    pub(super) command: CanvasSurfaceViewportCommand,
}
impl CanvasSurfaceViewportRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.viewport.request"
            && self.address.valid()
            && (1..=9007199254740991).contains(&self.revision)
            && self.command.valid()
    }
}

/// Schema-defined untrusted CanvasSurfaceViewportReply data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceViewportReply {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) address: CanvasSurfaceAddress,
    pub(super) revision: u64,
    pub(super) status: String,
    pub(super) message: String,
    pub(super) viewport: CanvasSurfaceViewportState,
}
impl CanvasSurfaceViewportReply {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.viewport.reply"
            && self.address.valid()
            && (1..=9007199254740991).contains(&self.revision)
            && self.status == "applied"
            && self.message.chars().count() <= 512
            && self.viewport.valid()
    }
}

fn valid_canvas_surface_capabilities_image_formats(value: &[String]) -> bool {
    if value.len() > 3 {
        return false;
    }
    for item in value {
        if !matches!(item.as_str(), "png" | "jpeg" | "webp") {
            return false;
        }
    }
    true
}

fn valid_canvas_surface_capabilities_viewport_commands(value: &[String]) -> bool {
    if value.len() > 4 {
        return false;
    }
    for item in value {
        if !matches!(item.as_str(), "fit" | "native" | "zoom" | "pan") {
            return false;
        }
    }
    true
}

fn valid_canvas_surface_capabilities_preview_formats(value: &[String]) -> bool {
    if value.len() > 2 {
        return false;
    }
    for item in value {
        if !matches!(item.as_str(), "png" | "jpeg") {
            return false;
        }
    }
    true
}

pub(super) fn valid_panel_session(value: &str) -> bool {
    (16..=80).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_canvas_surface_capabilities_reply_status(value: &str) -> bool {
    matches!(value, "available" | "unsupported" | "unavailable" | "error")
}

fn valid_surface_identity(value: &str, prefix: &str, maximum: usize) -> bool {
    value
        .strip_prefix(prefix)
        .is_some_and(|tail| !tail.is_empty() && !tail.chars().any(char::is_control))
        && value.chars().count() <= maximum
}

fn valid_canvas_surface_reply_status(value: &str) -> bool {
    matches!(value, "ready" | "applied" | "stale" | "cancelled" | "unavailable" | "error")
}
