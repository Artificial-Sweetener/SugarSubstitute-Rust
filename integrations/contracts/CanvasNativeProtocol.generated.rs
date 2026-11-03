// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from canvas.schema.json by scripts/integrations/generate-canvas-contract.ts.
// Schema SHA-256 (LF): d9d243f3ccecb0394dabbed54902e320aabacf2618c9922756a42f4fd9feeb9a

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

/// Schema-defined untrusted CanvasLayerCapabilities data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasLayerCapabilities {
    pub(super) display_output: bool,
    pub(super) insert_image_layer: bool,
    pub(super) position_image_layer: bool,
}
impl CanvasLayerCapabilities {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        true
    }
}

/// Schema-defined untrusted CanvasGeometryCapabilities data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasGeometryCapabilities {
    pub(super) display_output: bool,
    pub(super) insert_image_layer: bool,
    pub(super) position_image_layer: bool,
    pub(super) geometry: bool,
}
impl CanvasGeometryCapabilities {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        true
    }
}

/// Schema-defined untrusted CanvasEditingCapabilities data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasEditingCapabilities {
    pub(super) display_output: bool,
    pub(super) insert_image_layer: bool,
    pub(super) position_image_layer: bool,
    pub(super) geometry: bool,
    pub(super) editing: bool,
    pub(super) workflow_surfaces: bool,
}
impl CanvasEditingCapabilities {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        true
    }
}

/// Schema-defined untrusted CanvasCapabilities data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum CanvasCapabilities {
    Layer(CanvasLayerCapabilities),
    Geometry(CanvasGeometryCapabilities),
    Editing(CanvasEditingCapabilities),
}
impl CanvasCapabilities {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Layer(value) => value.valid(),
            Self::Geometry(value) => value.valid(),
            Self::Editing(value) => value.valid(),
        }
    }
}

/// Schema-defined untrusted CanvasDescriptor data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasDescriptor {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) capabilities: CanvasCapabilities,
}
impl CanvasDescriptor {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.id.encode_utf16().count() <= 128
            && self.label.encode_utf16().count() <= 128
            && self.capabilities.valid()
    }
}

/// Schema-defined untrusted CanvasDocument data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasDocument {
    pub(super) id: String,
    pub(super) geometry_revision: String,
    pub(super) width: u64,
    pub(super) height: u64,
}
impl CanvasDocument {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.id.encode_utf16().count() <= 128
            && self.geometry_revision.encode_utf16().count() <= 512
            && (1..=300000).contains(&self.width)
            && (1..=300000).contains(&self.height)
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

/// Schema-defined untrusted CanvasContext data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasContext {
    pub(super) canvas: CanvasDescriptor,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub(super) document: Option<CanvasDocument>,
}
impl CanvasContext {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.canvas.valid() && self.document.as_ref().is_none_or(CanvasDocument::valid)
    }
}

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

/// Schema-defined untrusted CanvasReadReply data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasReadReply {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) context: CanvasContext,
}
impl CanvasReadReply {
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

/// Schema-defined untrusted CanvasSourceTarget data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", deny_unknown_fields, rename_all_fields = "camelCase")]
pub(super) enum CanvasSourceTarget {
    #[serde(rename = "selection")]
    Selection {},
    #[serde(rename = "current-layer")]
    CurrentLayer {},
    #[serde(rename = "layer")]
    Layer { layer_id: String },
    #[serde(rename = "composition")]
    Composition {},
    #[serde(rename = "composition-from-layer")]
    CompositionFromLayer { layer_id: String },
}
impl CanvasSourceTarget {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Selection {} => true,
            Self::CurrentLayer {} => true,
            Self::Layer { layer_id } => layer_id.encode_utf16().count() <= 128,
            Self::Composition {} => true,
            Self::CompositionFromLayer { layer_id } => layer_id.encode_utf16().count() <= 128,
        }
    }
}

/// Schema-defined untrusted CanvasPixelRaster data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasPixelRaster {
    pub(super) width: u64,
    pub(super) height: u64,
    pub(super) format: String,
    pub(super) data: String,
}
impl CanvasPixelRaster {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (1..=65536).contains(&self.width)
            && (1..=65536).contains(&self.height)
            && matches!(self.format.as_str(), "rgba8" | "coverage8")
            && self.data.encode_utf16().count() <= 89478488
    }
}

/// Schema-defined untrusted CanvasSourceLayer data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSourceLayer {
    pub(super) id: String,
    pub(super) name: String,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub(super) parent_id: Option<String>,
    pub(super) depth: u64,
    pub(super) visible: bool,
}
impl CanvasSourceLayer {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.id.encode_utf16().count() <= 128
            && self.name.encode_utf16().count() <= 256
            && valid_optional_text(self.parent_id.as_deref(), 128)
            && (0..=128).contains(&self.depth)
    }
}

/// Schema-defined untrusted CanvasSourceSnapshot data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSourceSnapshot {
    pub(super) target: CanvasSourceTarget,
    pub(super) document: CanvasDocument,
    pub(super) revision: String,
    pub(super) pixels: CanvasPixelRaster,
}
impl CanvasSourceSnapshot {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.target.valid()
            && self.document.valid()
            && self.revision.encode_utf16().count() <= 512
            && self.pixels.valid()
    }
}

/// Schema-defined untrusted CanvasSourceCommand data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", deny_unknown_fields, rename_all_fields = "camelCase")]
pub(super) enum CanvasSourceCommand {
    #[serde(rename = "context")]
    Context {},
    #[serde(rename = "capture")]
    Capture { document: CanvasDocument, targets: Vec<CanvasSourceTarget>, maximum_size: u64 },
    #[serde(rename = "watch")]
    Watch { enabled: bool },
}
impl CanvasSourceCommand {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Context {} => true,
            Self::Capture { document, targets, maximum_size } => {
                document.valid()
                    && targets.len() <= 64
                    && targets.iter().all(CanvasSourceTarget::valid)
                    && (0..=1024).contains(maximum_size)
            }
            Self::Watch { enabled: _ } => true,
        }
    }
}

/// Schema-defined untrusted CanvasSourceResult data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", deny_unknown_fields, rename_all_fields = "camelCase")]
pub(super) enum CanvasSourceResult {
    #[serde(rename = "context")]
    Context {
        supported: bool,
        #[serde(deserialize_with = "deserialize_required_option")]
        document: Option<CanvasDocument>,
        revision: String,
        targets: Vec<String>,
        layers: Vec<CanvasSourceLayer>,
        #[serde(deserialize_with = "deserialize_required_option")]
        current_layer_id: Option<String>,
        observes_changes: bool,
    },
    #[serde(rename = "captured")]
    Captured { snapshots: Vec<CanvasSourceSnapshot> },
    #[serde(rename = "watching")]
    Watching { supported: bool },
    #[serde(rename = "failure")]
    Failure { status: String, message: String },
}
impl CanvasSourceResult {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Context {
                supported: _,
                document,
                revision,
                targets,
                layers,
                current_layer_id,
                observes_changes: _,
            } => {
                document.as_ref().is_none_or(CanvasDocument::valid)
                    && revision.encode_utf16().count() <= 512
                    && valid_canvas_source_result_context_targets(targets)
                    && layers.len() <= 4096
                    && layers.iter().all(CanvasSourceLayer::valid)
                    && valid_optional_text(current_layer_id.as_deref(), 128)
            }
            Self::Captured { snapshots } => {
                snapshots.len() <= 64 && snapshots.iter().all(CanvasSourceSnapshot::valid)
            }
            Self::Watching { supported: _ } => true,
            Self::Failure { status, message } => {
                valid_canvas_source_result_failure_status(status)
                    && message.encode_utf16().count() <= 512
            }
        }
    }
}

/// Schema-defined untrusted CanvasSourceRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSourceRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) command: CanvasSourceCommand,
}
impl CanvasSourceRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "source.request"
            && self.command.valid()
    }
}

/// Schema-defined untrusted CanvasSourceReply data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSourceReply {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) result: CanvasSourceResult,
}
impl CanvasSourceReply {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "source.reply"
            && self.result.valid()
    }
}

/// Schema-defined untrusted CanvasSourceChanged data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSourceChanged {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) change: CanvasSourceChange,
}
impl CanvasSourceChanged {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && self.request_id == 0
            && self.r#type == "source.changed"
            && self.change.valid()
    }
}

/// Schema-defined untrusted CanvasSourceChange data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSourceChange {
    #[serde(deserialize_with = "deserialize_required_option")]
    pub(super) document_id: Option<String>,
    pub(super) revision: String,
}
impl CanvasSourceChange {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        valid_optional_text(self.document_id.as_deref(), 128)
            && self.revision.encode_utf16().count() <= 512
    }
}

/// Schema-defined untrusted CanvasDimensions data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasDimensions {
    pub(super) width: u64,
    pub(super) height: u64,
}
impl CanvasDimensions {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (1..=300000).contains(&self.width) && (1..=300000).contains(&self.height)
    }
}

/// Schema-defined untrusted CanvasSelectionBounds data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasSelectionBounds {
    pub(super) left: i64,
    pub(super) top: i64,
    pub(super) width: u64,
    pub(super) height: u64,
}
impl CanvasSelectionBounds {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (-300000..=300000).contains(&self.left)
            && (-300000..=300000).contains(&self.top)
            && (1..=300000).contains(&self.width)
            && (1..=300000).contains(&self.height)
    }
}

/// Schema-defined untrusted CanvasGeometryCommand data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", deny_unknown_fields, rename_all_fields = "camelCase")]
pub(super) enum CanvasGeometryCommand {
    #[serde(rename = "context")]
    Context {},
    #[serde(rename = "resize")]
    Resize { document: CanvasDocument, size: CanvasDimensions },
}
impl CanvasGeometryCommand {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Context {} => true,
            Self::Resize { document, size } => document.valid() && size.valid(),
        }
    }
}

/// Schema-defined untrusted CanvasGeometryResult data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", deny_unknown_fields, rename_all_fields = "camelCase")]
pub(super) enum CanvasGeometryResult {
    #[serde(rename = "context")]
    Context {
        supported: bool,
        resize: bool,
        selection_supported: bool,
        #[serde(deserialize_with = "deserialize_required_option")]
        document: Option<CanvasDocument>,
        #[serde(deserialize_with = "deserialize_required_option")]
        selection: Option<CanvasSelectionBounds>,
    },
    #[serde(rename = "resized")]
    Resized { document: CanvasDocument },
    #[serde(rename = "failure")]
    Failure { status: String, message: String },
}
impl CanvasGeometryResult {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Context {
                supported: _,
                resize: _,
                selection_supported: _,
                document,
                selection,
            } => {
                document.as_ref().is_none_or(CanvasDocument::valid)
                    && selection.as_ref().is_none_or(CanvasSelectionBounds::valid)
            }
            Self::Resized { document } => document.valid(),
            Self::Failure { status, message } => {
                valid_canvas_geometry_result_failure_status(status)
                    && message.encode_utf16().count() <= 512
            }
        }
    }
}

/// Schema-defined untrusted CanvasGeometryRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasGeometryRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) command: CanvasGeometryCommand,
}
impl CanvasGeometryRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "geometry.request"
            && self.command.valid()
    }
}

/// Schema-defined untrusted CanvasGeometryReply data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasGeometryReply {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) result: CanvasGeometryResult,
}
impl CanvasGeometryReply {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "geometry.reply"
            && self.result.valid()
    }
}

/// Schema-defined untrusted CanvasToolId token; decoding admits only the listed values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum CanvasToolId {
    PanZoom,
    Move,
    Brush,
    Eraser,
    SelectRectangle,
    SelectEllipse,
    SelectLasso,
    MaskRectangle,
    MaskEllipse,
    MaskLasso,
    Transform,
    SharedEdges,
}

/// Schema-defined untrusted CanvasBrushOptions data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasBrushOptions {
    pub(super) size: u64,
    pub(super) hardness: u64,
    pub(super) opacity: u64,
}
impl CanvasBrushOptions {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (1..=1000).contains(&self.size)
            && (0..=100).contains(&self.hardness)
            && (0..=100).contains(&self.opacity)
    }
}

/// Schema-defined untrusted CanvasEditCommand data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", deny_unknown_fields, rename_all_fields = "camelCase")]
pub(super) enum CanvasEditCommand {
    #[serde(rename = "state")]
    State {},
    #[serde(rename = "tool")]
    Tool { tool: CanvasToolId },
    #[serde(rename = "options")]
    Options { tool: String, options: CanvasBrushOptions },
    #[serde(rename = "history")]
    History { action: String },
    #[serde(rename = "commit")]
    Commit {},
    #[serde(rename = "cancel")]
    Cancel {},
    #[serde(rename = "nudge")]
    Nudge { dx: i64, dy: i64 },
    #[serde(rename = "selection")]
    Selection { action: String },
    #[serde(rename = "modify")]
    Modify { target: String, operation: String, pixels: u64, phase: String },
    #[serde(rename = "mode")]
    Mode { mode: String },
    #[serde(rename = "transform")]
    Transform { action: String },
    #[serde(rename = "session")]
    Session { action: String },
    #[serde(rename = "navigation")]
    Navigation { held: bool },
}
impl CanvasEditCommand {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::State {} => true,
            Self::Tool { tool: _ } => true,
            Self::Options { tool, options } => {
                matches!(tool.as_str(), "brush" | "eraser") && options.valid()
            }
            Self::History { action } => matches!(action.as_str(), "undo" | "redo"),
            Self::Commit {} => true,
            Self::Cancel {} => true,
            Self::Nudge { dx, dy } => (-100..=100).contains(dx) && (-100..=100).contains(dy),
            Self::Selection { action } => matches!(action.as_str(), "deselect" | "clear"),
            Self::Modify { target, operation, pixels, phase } => {
                matches!(target.as_str(), "selection" | "mask")
                    && matches!(operation.as_str(), "expand" | "contract" | "feather")
                    && (1..=999).contains(pixels)
                    && matches!(phase.as_str(), "preview" | "apply" | "cancel")
            }
            Self::Mode { mode } => matches!(mode.as_str(), "none" | "coverage"),
            Self::Transform { action } => valid_canvas_edit_command_transform_action(action),
            Self::Session { action } => {
                matches!(action.as_str(), "undo" | "redo" | "apply" | "cancel")
            }
            Self::Navigation { held: _ } => true,
        }
    }
}

/// Schema-defined untrusted CanvasEditTool data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasEditTool {
    pub(super) id: CanvasToolId,
    pub(super) available: bool,
}
impl CanvasEditTool {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        true
    }
}

/// Schema-defined untrusted CanvasEditToolOptions data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasEditToolOptions {
    pub(super) brush: CanvasBrushOptions,
    pub(super) eraser: CanvasBrushOptions,
}
impl CanvasEditToolOptions {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.brush.valid() && self.eraser.valid()
    }
}

/// Schema-defined untrusted CanvasEditSelectionBounds data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasEditSelectionBounds {
    pub(super) left: f64,
    pub(super) top: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}
impl CanvasEditSelectionBounds {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (-1000000.0..=1000000.0).contains(&self.left)
            && (-1000000.0..=1000000.0).contains(&self.top)
            && (0.0..=1000000.0).contains(&self.width)
            && self.width > 0.0
            && (0.0..=1000000.0).contains(&self.height)
            && self.height > 0.0
    }
}

/// Schema-defined untrusted CanvasEditSelection data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasEditSelection {
    pub(super) bounds: CanvasEditSelectionBounds,
}
impl CanvasEditSelection {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.bounds.valid()
    }
}

/// Schema-defined untrusted CanvasEditHistory data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasEditHistory {
    pub(super) undo: bool,
    pub(super) redo: bool,
}
impl CanvasEditHistory {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        true
    }
}

/// Schema-defined untrusted CanvasEditSession data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasEditSession {
    pub(super) kind: String,
    pub(super) undo: bool,
    pub(super) redo: bool,
}
impl CanvasEditSession {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        valid_canvas_edit_session_kind(&self.kind)
    }
}

/// Schema-defined untrusted CanvasEditState data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasEditState {
    pub(super) revision: u64,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub(super) document: Option<String>,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub(super) active_mask: Option<String>,
    pub(super) tool: CanvasToolId,
    pub(super) tools: Vec<CanvasEditTool>,
    pub(super) options: CanvasEditToolOptions,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub(super) selection: Option<CanvasEditSelection>,
    pub(super) history: CanvasEditHistory,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub(super) session: Option<CanvasEditSession>,
    pub(super) mode: String,
}
impl CanvasEditState {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        (0..=9007199254740991).contains(&self.revision)
            && valid_optional_text(self.document.as_deref(), 128)
            && valid_optional_text(self.active_mask.as_deref(), 128)
            && self.tools.len() <= 12
            && self.tools.iter().all(CanvasEditTool::valid)
            && self.options.valid()
            && self.selection.as_ref().is_none_or(CanvasEditSelection::valid)
            && self.history.valid()
            && self.session.as_ref().is_none_or(CanvasEditSession::valid)
            && matches!(self.mode.as_str(), "none" | "coverage")
    }
}

/// Schema-defined untrusted CanvasEditResult data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", deny_unknown_fields, rename_all_fields = "camelCase")]
pub(super) enum CanvasEditResult {
    #[serde(rename = "state")]
    State { state: CanvasEditState },
    #[serde(rename = "failure")]
    Failure { status: String, message: String },
}
impl CanvasEditResult {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::State { state } => state.valid(),
            Self::Failure { status, message } => {
                valid_canvas_edit_result_failure_status(status)
                    && message.encode_utf16().count() <= 512
            }
        }
    }
}

/// Schema-defined untrusted CanvasEditRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasEditRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) command: CanvasEditCommand,
}
impl CanvasEditRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "edit.request"
            && self.command.valid()
    }
}

/// Schema-defined untrusted CanvasEditReply data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasEditReply {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) result: CanvasEditResult,
}
impl CanvasEditReply {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "edit.reply"
            && self.result.valid()
    }
}

/// Schema-defined untrusted CanvasEditChanged data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasEditChanged {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) state: CanvasEditState,
}
impl CanvasEditChanged {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && self.request_id == 0
            && self.r#type == "edit.changed"
            && self.state.valid()
    }
}

/// Schema-defined untrusted CanvasWorkflowSurfaceBase data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", deny_unknown_fields, rename_all_fields = "camelCase")]
pub(super) enum CanvasWorkflowSurfaceBase {
    #[serde(rename = "file")]
    File { file: CanvasSurfaceFileReference },
    #[serde(rename = "blank")]
    Blank { width: u64, height: u64 },
}
impl CanvasWorkflowSurfaceBase {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::File { file } => file.valid(),
            Self::Blank { width, height } => {
                (1..=300000).contains(width) && (1..=300000).contains(height)
            }
        }
    }
}

/// Schema-defined untrusted CanvasWorkflowMask data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasWorkflowMask {
    pub(super) key: String,
    pub(super) label: String,
    pub(super) color: String,
    pub(super) opacity: f64,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub(super) initial: Option<CanvasSurfaceFileReference>,
    pub(super) channel: String,
}
impl CanvasWorkflowMask {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.key.encode_utf16().count() <= 256
            && self.label.encode_utf16().count() <= 256
            && valid_color(&self.color)
            && (0.0..=1.0).contains(&self.opacity)
            && self.initial.as_ref().is_none_or(CanvasSurfaceFileReference::valid)
            && matches!(self.channel.as_str(), "red" | "green" | "blue" | "alpha")
    }
}

/// Schema-defined untrusted CanvasWorkflowSurface data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasWorkflowSurface {
    pub(super) key: String,
    pub(super) label: String,
    pub(super) base: CanvasWorkflowSurfaceBase,
    pub(super) masks: Vec<CanvasWorkflowMask>,
}
impl CanvasWorkflowSurface {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.key.encode_utf16().count() <= 256
            && self.label.encode_utf16().count() <= 256
            && self.base.valid()
            && self.masks.len() <= 32
            && self.masks.iter().all(CanvasWorkflowMask::valid)
    }
}

/// Schema-defined untrusted CanvasWorkflowAppearance data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasWorkflowAppearance {
    pub(super) surface: String,
    pub(super) mask: String,
    pub(super) color: String,
    pub(super) opacity: f64,
}
impl CanvasWorkflowAppearance {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.surface.encode_utf16().count() <= 256
            && self.mask.encode_utf16().count() <= 256
            && valid_color(&self.color)
            && (0.0..=1.0).contains(&self.opacity)
    }
}

/// Schema-defined untrusted CanvasWorkflowResizeMode data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", deny_unknown_fields, rename_all_fields = "camelCase")]
pub(super) enum CanvasWorkflowResizeMode {
    #[serde(rename = "canvas")]
    Canvas { anchor: String },
    #[serde(rename = "scale")]
    Scale { resampling: String },
}
impl CanvasWorkflowResizeMode {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Canvas { anchor } => valid_canvas_workflow_resize_mode_canvas_anchor(anchor),
            Self::Scale { resampling } => matches!(resampling.as_str(), "nearest" | "smooth"),
        }
    }
}

/// Schema-defined untrusted CanvasWorkflowCommand data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", deny_unknown_fields, rename_all_fields = "camelCase")]
pub(super) enum CanvasWorkflowCommand {
    #[serde(rename = "sync")]
    Sync { workflow: String, surfaces: Vec<CanvasWorkflowSurface> },
    #[serde(rename = "activate")]
    Activate {
        workflow: String,
        route: String,
        #[serde(deserialize_with = "deserialize_required_option")]
        surface: Option<String>,
        #[serde(deserialize_with = "deserialize_required_option")]
        mask: Option<String>,
    },
    #[serde(rename = "appearance")]
    Appearance { workflow: String, masks: Vec<CanvasWorkflowAppearance> },
    #[serde(rename = "resize")]
    Resize {
        workflow: String,
        surface: String,
        width: u64,
        height: u64,
        mode: CanvasWorkflowResizeMode,
    },
    #[serde(rename = "hover")]
    Hover {
        workflow: String,
        surface: String,
        #[serde(deserialize_with = "deserialize_required_option")]
        mask: Option<String>,
    },
}
impl CanvasWorkflowCommand {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Sync { workflow, surfaces } => {
                workflow.encode_utf16().count() <= 256
                    && surfaces.len() <= 64
                    && surfaces.iter().all(CanvasWorkflowSurface::valid)
            }
            Self::Activate { workflow, route, surface, mask } => {
                workflow.encode_utf16().count() <= 256
                    && matches!(route.as_str(), "input" | "output")
                    && valid_optional_text(surface.as_deref(), 256)
                    && valid_optional_text(mask.as_deref(), 256)
            }
            Self::Appearance { workflow, masks } => {
                workflow.encode_utf16().count() <= 256
                    && !masks.is_empty()
                    && masks.len() <= 2048
                    && masks.iter().all(CanvasWorkflowAppearance::valid)
            }
            Self::Resize { workflow, surface, width, height, mode } => {
                workflow.encode_utf16().count() <= 256
                    && surface.encode_utf16().count() <= 256
                    && (1..=300000).contains(width)
                    && (1..=300000).contains(height)
                    && mode.valid()
            }
            Self::Hover { workflow, surface, mask } => {
                workflow.encode_utf16().count() <= 256
                    && surface.encode_utf16().count() <= 256
                    && valid_optional_text(mask.as_deref(), 256)
            }
        }
    }
}

/// Schema-defined untrusted CanvasWorkflowMaskLayer data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasWorkflowMaskLayer {
    pub(super) key: String,
    pub(super) layer_id: String,
}
impl CanvasWorkflowMaskLayer {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.key.encode_utf16().count() <= 256 && self.layer_id.encode_utf16().count() <= 128
    }
}

/// Schema-defined untrusted CanvasWorkflowSyncedSurface data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasWorkflowSyncedSurface {
    pub(super) key: String,
    pub(super) document: CanvasDocument,
    pub(super) masks: Vec<CanvasWorkflowMaskLayer>,
}
impl CanvasWorkflowSyncedSurface {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.key.encode_utf16().count() <= 256
            && self.document.valid()
            && self.masks.len() <= 32
            && self.masks.iter().all(CanvasWorkflowMaskLayer::valid)
    }
}

/// Schema-defined untrusted CanvasWorkflowResult data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", deny_unknown_fields, rename_all_fields = "camelCase")]
pub(super) enum CanvasWorkflowResult {
    #[serde(rename = "synced")]
    Synced { surfaces: Vec<CanvasWorkflowSyncedSurface> },
    #[serde(rename = "activated")]
    Activated {},
    #[serde(rename = "applied")]
    Applied {},
    #[serde(rename = "failure")]
    Failure { status: String, message: String },
}
impl CanvasWorkflowResult {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Synced { surfaces } => {
                surfaces.len() <= 64 && surfaces.iter().all(CanvasWorkflowSyncedSurface::valid)
            }
            Self::Activated {} => true,
            Self::Applied {} => true,
            Self::Failure { status, message } => {
                valid_canvas_workflow_result_failure_status(status)
                    && message.encode_utf16().count() <= 512
            }
        }
    }
}

/// Schema-defined untrusted CanvasWorkflowRequest data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasWorkflowRequest {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) command: CanvasWorkflowCommand,
}
impl CanvasWorkflowRequest {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "workflow.request"
            && self.command.valid()
    }
}

/// Schema-defined untrusted CanvasWorkflowReply data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct CanvasWorkflowReply {
    pub(super) protocol: String,
    pub(super) version: u64,
    pub(super) session: String,
    pub(super) request_id: u64,
    pub(super) r#type: String,
    pub(super) result: CanvasWorkflowResult,
}
impl CanvasWorkflowReply {
    /// Reject values outside the exact schema-defined bounds.
    pub(super) fn valid(&self) -> bool {
        self.protocol == "sugarcubes.canvas"
            && self.version == 3
            && valid_panel_session(&self.session)
            && (1..=2147483647).contains(&self.request_id)
            && self.r#type == "workflow.reply"
            && self.result.valid()
    }
}

fn valid_canvas_failure_status(value: &str) -> bool {
    matches!(value, "unavailable" | "unsupported" | "stale" | "cancelled" | "error")
}

pub(super) fn valid_panel_session(value: &str) -> bool {
    (16..=80).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_optional_text(value: Option<&str>, maximum: usize) -> bool {
    value.is_none_or(|value| value.encode_utf16().count() <= maximum)
}

fn valid_canvas_source_result_context_targets(value: &[String]) -> bool {
    if value.len() > 5 {
        return false;
    }
    for item in value {
        if !matches!(
            item.as_str(),
            "selection" | "current-layer" | "layer" | "composition" | "composition-from-layer"
        ) {
            return false;
        }
    }
    true
}

fn valid_canvas_source_result_failure_status(value: &str) -> bool {
    matches!(value, "unavailable" | "unsupported" | "stale" | "cancelled" | "error")
}

fn valid_canvas_geometry_result_failure_status(value: &str) -> bool {
    matches!(value, "unavailable" | "unsupported" | "stale" | "cancelled" | "error")
}

fn valid_canvas_edit_command_transform_action(value: &str) -> bool {
    matches!(value, "rotate-left" | "rotate-right" | "flip-horizontal" | "flip-vertical")
}

fn valid_canvas_edit_session_kind(value: &str) -> bool {
    matches!(value, "move" | "transform" | "shared-edges" | "modify")
}

fn valid_canvas_edit_result_failure_status(value: &str) -> bool {
    matches!(value, "unavailable" | "unsupported" | "stale" | "cancelled" | "error")
}

fn valid_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}

fn valid_canvas_workflow_resize_mode_canvas_anchor(value: &str) -> bool {
    matches!(
        value,
        "top-left"
            | "top"
            | "top-right"
            | "left"
            | "center"
            | "right"
            | "bottom-left"
            | "bottom"
            | "bottom-right"
    )
}

fn valid_canvas_workflow_result_failure_status(value: &str) -> bool {
    matches!(value, "unavailable" | "unsupported" | "stale" | "cancelled" | "error")
}

/// Preserve a required nullable field instead of defaulting a missing member to null.
fn deserialize_required_option<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    serde::Deserialize::deserialize(deserializer)
}
