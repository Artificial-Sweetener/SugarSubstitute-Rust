// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from canvas.schema.json by scripts/integrations/generate-canvas-contract.ts.
// Schema SHA-256 (LF): 066f625fd11d756acf0ba9a67121624ce322032fa3c6bfcdf8e782e99be1db96

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
    pub(super) input: bool,
    #[serde(
        default,
        deserialize_with = "deserialize_present_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) backdrop: Option<CanvasSurfaceBackdropSupport>,
}
impl CanvasSurfaceCapabilities {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (1..=3).contains(&self.schema)
            && valid_canvas_surface_capabilities_image_formats(&self.image_formats)
            && valid_canvas_surface_capabilities_viewport_commands(&self.viewport_commands)
            && valid_canvas_surface_capabilities_preview_formats(&self.preview_formats)
            && self.backdrop.as_ref().is_none_or(CanvasSurfaceBackdropSupport::valid)
    }
}

/// Schema-defined untrusted CanvasSurfaceBackdropSupport data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceBackdropSupport {
    pub(super) schema: u64,
}
impl CanvasSurfaceBackdropSupport {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.schema == 1
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
            && (1..=3).contains(&self.schema)
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

/// Schema-defined untrusted CanvasSurfaceOcclusion data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceOcclusion {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}
impl CanvasSurfaceOcclusion {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (-1000000.0..=1000000.0).contains(&self.x)
            && (-1000000.0..=1000000.0).contains(&self.y)
            && (0.0..=1000000.0).contains(&self.width)
            && self.width > 0.0
            && (0.0..=1000000.0).contains(&self.height)
            && self.height > 0.0
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
    #[serde(
        default,
        deserialize_with = "deserialize_present_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) occlusions: Option<Vec<CanvasSurfaceOcclusion>>,
    #[serde(
        default,
        deserialize_with = "deserialize_present_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) modal: Option<bool>,
    #[serde(
        default,
        deserialize_with = "deserialize_present_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) backdrops: Option<Vec<CanvasSurfaceBackdrop>>,
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
            && valid_canvas_surface_geometry_occlusions(self.occlusions.as_deref())
            && valid_canvas_surface_geometry_backdrops(self.backdrops.as_deref())
    }
}

/// Schema-defined untrusted CanvasSurfaceBackdrop data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceBackdrop {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) corner_radius: f64,
    pub(super) sigma: f64,
}
impl CanvasSurfaceBackdrop {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (-1000000.0..=1000000.0).contains(&self.x)
            && (-1000000.0..=1000000.0).contains(&self.y)
            && (0.0..=1000000.0).contains(&self.width)
            && self.width > 0.0
            && (0.0..=1000000.0).contains(&self.height)
            && self.height > 0.0
            && (0.0..=1000000.0).contains(&self.corner_radius)
            && (0.0..=64.0).contains(&self.sigma)
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

/// Schema-defined untrusted CanvasSurfaceComparison data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceComparison {
    pub(super) item_id: String,
    pub(super) file: CanvasSurfaceFileReference,
}
impl CanvasSurfaceComparison {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        valid_surface_identity(&self.item_id, "result:", 4096) && self.file.valid()
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
    #[serde(
        default,
        deserialize_with = "deserialize_present_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) comparison: Option<CanvasSurfaceComparison>,
    #[serde(
        default,
        deserialize_with = "deserialize_present_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) inspection: Option<CanvasSurfaceInspection>,
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
            && self.comparison.as_ref().is_none_or(CanvasSurfaceComparison::valid)
            && self.inspection.as_ref().is_none_or(CanvasSurfaceInspection::valid)
    }
}

/// Schema-defined untrusted CanvasSurfaceOverviewRect data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceOverviewRect {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}
impl CanvasSurfaceOverviewRect {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (-1000000.0..=1000000.0).contains(&self.x)
            && (-1000000.0..=1000000.0).contains(&self.y)
            && (0.0..=1000000.0).contains(&self.width)
            && self.width > 0.0
            && (0.0..=1000000.0).contains(&self.height)
            && self.height > 0.0
    }
}

/// Schema-defined untrusted CanvasSurfaceOverviewCell data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceOverviewCell {
    pub(super) item_id: String,
    pub(super) file: CanvasSurfaceFileReference,
    pub(super) rect: CanvasSurfaceOverviewRect,
}
impl CanvasSurfaceOverviewCell {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        valid_surface_identity(&self.item_id, "result:", 1024)
            && self.file.valid()
            && self.rect.valid()
    }
}

/// Schema-defined untrusted CanvasSurfaceOverviewRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceOverviewRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) address: CanvasSurfaceAddress,
    pub(super) revision: u64,
    pub(super) cells: Vec<CanvasSurfaceOverviewCell>,
}
impl CanvasSurfaceOverviewRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.overview"
            && self.address.valid()
            && (1..=9007199254740991).contains(&self.revision)
            && !self.cells.is_empty()
            && self.cells.len() <= 8
            && self.cells.iter().all(CanvasSurfaceOverviewCell::valid)
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
    #[serde(rename = "wipe")]
    Wipe { orientation: String, split: f64 },
    #[serde(rename = "inspect")]
    Inspect { inspection: CanvasSurfaceInspection },
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
            Self::Wipe { orientation, split } => {
                matches!(orientation.as_str(), "vertical" | "horizontal")
                    && (0.0..=1.0).contains(split)
            }
            Self::Inspect { inspection } => inspection.valid(),
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

/// Schema-defined untrusted CanvasSurfaceViewportCenter data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceViewportCenter {
    pub(super) x: f64,
    pub(super) y: f64,
}
impl CanvasSurfaceViewportCenter {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

/// Schema-defined untrusted CanvasSurfaceInspectionSpan data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceInspectionSpan {
    pub(super) x: f64,
    pub(super) y: f64,
}
impl CanvasSurfaceInspectionSpan {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (0.0..=1000000000.0).contains(&self.x)
            && self.x > 0.0
            && (0.0..=1000000000.0).contains(&self.y)
            && self.y > 0.0
    }
}

/// Schema-defined untrusted CanvasSurfaceInspection data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceInspection {
    pub(super) center: CanvasSurfaceViewportCenter,
    pub(super) span: CanvasSurfaceInspectionSpan,
}
impl CanvasSurfaceInspection {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.center.valid() && self.span.valid()
    }
}

/// Schema-defined untrusted CanvasSurfaceViewportState data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceViewportState {
    pub(super) mode: String,
    pub(super) physical_scale: f64,
    pub(super) dimensions: CanvasSurfaceImageDimensions,
    #[serde(
        default,
        deserialize_with = "deserialize_present_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) secondary_dimensions: Option<CanvasSurfaceImageDimensions>,
    #[serde(
        default,
        deserialize_with = "deserialize_present_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) center: Option<CanvasSurfaceViewportCenter>,
}
impl CanvasSurfaceViewportState {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        matches!(self.mode.as_str(), "fit" | "native" | "custom")
            && (1e-9..=16384.0).contains(&self.physical_scale)
            && self.dimensions.valid()
            && self.secondary_dimensions.as_ref().is_none_or(CanvasSurfaceImageDimensions::valid)
            && self.center.as_ref().is_none_or(CanvasSurfaceViewportCenter::valid)
    }
}

/// Schema-defined untrusted CanvasSurfaceCameraChange data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceCameraChange {
    pub(super) address: CanvasSurfaceAddress,
    pub(super) sequence: u64,
    pub(super) selection_revision: u64,
    pub(super) viewport: CanvasSurfaceViewportState,
}
impl CanvasSurfaceCameraChange {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.address.valid()
            && (1..=9007199254740991).contains(&self.sequence)
            && (1..=9007199254740991).contains(&self.selection_revision)
            && self.viewport.valid()
    }
}

/// Schema-defined untrusted CanvasSurfaceCameraChanged data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfaceCameraChanged {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) schema: u64,
    pub(super) change: CanvasSurfaceCameraChange,
}
impl CanvasSurfaceCameraChanged {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && self.request_id == 0
            && self.r#type == "surface.camera.changed"
            && self.schema == 2
            && self.change.valid()
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

/// Schema-defined untrusted CanvasSurfacePreviewIdentity data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfacePreviewIdentity {
    pub(super) generation: u64,
    pub(super) prompt_id: String,
    pub(super) source_id: String,
    pub(super) run_order: u64,
    pub(super) sequence: u64,
    pub(super) token: String,
}
impl CanvasSurfacePreviewIdentity {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (0..=9007199254740991).contains(&self.generation)
            && self.prompt_id.chars().count() <= 256
            && self.source_id.chars().count() <= 4096
            && (1..=9007199254740991).contains(&self.run_order)
            && (1..=9007199254740991).contains(&self.sequence)
            && self.token.chars().count() <= 24
    }
}

/// Schema-defined untrusted CanvasSurfacePreviewBeginRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfacePreviewBeginRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) address: CanvasSurfaceAddress,
    pub(super) revision: u64,
    pub(super) preview: CanvasSurfacePreviewIdentity,
    pub(super) byte_length: u64,
    pub(super) format: String,
}
impl CanvasSurfacePreviewBeginRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.preview.begin"
            && self.address.valid()
            && (1..=9007199254740991).contains(&self.revision)
            && self.preview.valid()
            && (1..=67108864).contains(&self.byte_length)
            && matches!(self.format.as_str(), "png" | "jpeg")
    }
}

/// Schema-defined untrusted CanvasSurfacePreviewChunkRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfacePreviewChunkRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) address: CanvasSurfaceAddress,
    pub(super) revision: u64,
    pub(super) token: String,
    pub(super) lease_revision: u64,
    pub(super) offset: u64,
    pub(super) data: String,
}
impl CanvasSurfacePreviewChunkRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.preview.chunk"
            && self.address.valid()
            && (1..=9007199254740991).contains(&self.revision)
            && self.token.chars().count() <= 24
            && (1..=9007199254740991).contains(&self.lease_revision)
            && (0..=67108863).contains(&self.offset)
            && valid_bounded_base64(&self.data, 10924)
    }
}

/// Schema-defined untrusted CanvasSurfacePreviewEndRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfacePreviewEndRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) address: CanvasSurfaceAddress,
    pub(super) revision: u64,
    pub(super) token: String,
    pub(super) lease_revision: u64,
}
impl CanvasSurfacePreviewEndRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.preview.end"
            && self.address.valid()
            && (1..=9007199254740991).contains(&self.revision)
            && self.token.chars().count() <= 24
            && (1..=9007199254740991).contains(&self.lease_revision)
    }
}

/// Schema-defined untrusted CanvasSurfacePreviewWithdrawRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfacePreviewWithdrawRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) address: CanvasSurfaceAddress,
    pub(super) revision: u64,
    pub(super) token: String,
    pub(super) lease_revision: u64,
}
impl CanvasSurfacePreviewWithdrawRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.preview.withdraw"
            && self.address.valid()
            && (1..=9007199254740991).contains(&self.revision)
            && self.token.chars().count() <= 24
            && (1..=9007199254740991).contains(&self.lease_revision)
    }
}

/// Schema-defined untrusted CanvasSurfacePresentationRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfacePresentationRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) schema: u64,
    pub(super) enabled: bool,
}
impl CanvasSurfacePresentationRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.presentation.request"
            && self.schema == 1
    }
}

/// Schema-defined untrusted CanvasSurfacePresentationReply data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfacePresentationReply {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) schema: u64,
    pub(super) enabled: bool,
    pub(super) status: String,
    pub(super) message: String,
}
impl CanvasSurfacePresentationReply {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "surface.presentation.reply"
            && self.schema == 1
            && valid_canvas_surface_presentation_reply_status(&self.status)
            && self.message.chars().count() <= 512
    }
}

/// Schema-defined untrusted CanvasSurfacePresentationDisplay data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", deny_unknown_fields, rename_all_fields = "camelCase")]
pub(super) enum CanvasSurfacePresentationDisplay {
    #[serde(rename = "empty")]
    Empty {},
    #[serde(rename = "preview")]
    Preview { token: String, lease_revision: u64, viewport: CanvasSurfaceViewportState },
    #[serde(rename = "final")]
    Final { selection_revision: u64, viewport: CanvasSurfaceViewportState },
    #[serde(rename = "overview")]
    Overview { selection_revision: u64 },
}
impl CanvasSurfacePresentationDisplay {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Empty {} => true,
            Self::Preview { token, lease_revision, viewport } => {
                token.chars().count() <= 24
                    && (1..=9007199254740991).contains(lease_revision)
                    && viewport.valid()
            }
            Self::Final { selection_revision, viewport } => {
                (1..=9007199254740991).contains(selection_revision) && viewport.valid()
            }
            Self::Overview { selection_revision } => {
                (1..=9007199254740991).contains(selection_revision)
            }
        }
    }
}

/// Schema-defined untrusted CanvasSurfacePresentationChange data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfacePresentationChange {
    pub(super) address: CanvasSurfaceAddress,
    pub(super) sequence: u64,
    pub(super) token: String,
    pub(super) lease_revision: u64,
    pub(super) status: String,
    pub(super) displayed: CanvasSurfacePresentationDisplay,
    pub(super) message: String,
}
impl CanvasSurfacePresentationChange {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.address.valid()
            && (1..=9007199254740991).contains(&self.sequence)
            && self.token.chars().count() <= 24
            && (1..=9007199254740991).contains(&self.lease_revision)
            && matches!(self.status.as_str(), "presented" | "failed" | "cancelled")
            && self.displayed.valid()
            && self.message.chars().count() <= 512
    }
}

/// Schema-defined untrusted CanvasSurfacePresentationChanged data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSurfacePresentationChanged {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) schema: u64,
    pub(super) change: CanvasSurfacePresentationChange,
}
impl CanvasSurfacePresentationChanged {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && self.request_id == 0
            && self.r#type == "surface.presentation.changed"
            && self.schema == 1
            && self.change.valid()
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
    if value.len() > 6 {
        return false;
    }
    for item in value {
        if !matches!(item.as_str(), "fit" | "native" | "zoom" | "pan" | "wipe" | "inspect") {
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

fn valid_canvas_surface_geometry_occlusions(value: Option<&[CanvasSurfaceOcclusion]>) -> bool {
    value.is_none_or(|items| items.len() <= 64 && items.iter().all(CanvasSurfaceOcclusion::valid))
}

fn valid_canvas_surface_geometry_backdrops(value: Option<&[CanvasSurfaceBackdrop]>) -> bool {
    value.is_none_or(|items| items.len() <= 2 && items.iter().all(CanvasSurfaceBackdrop::valid))
}

fn valid_canvas_surface_reply_status(value: &str) -> bool {
    matches!(value, "ready" | "applied" | "stale" | "cancelled" | "unavailable" | "error")
}

fn valid_bounded_base64(value: &str, maximum: usize) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() > maximum || !bytes.len().is_multiple_of(4) {
        return false;
    }
    let padding = if value.ends_with("==") { 2 } else { usize::from(value.ends_with('=')) };
    bytes[..bytes.len() - padding]
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'))
}

fn valid_canvas_surface_presentation_reply_status(value: &str) -> bool {
    matches!(value, "available" | "unsupported" | "unavailable" | "error")
}

/// Decode an optional member only when present, rejecting an explicit null.
fn deserialize_present_option<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}
