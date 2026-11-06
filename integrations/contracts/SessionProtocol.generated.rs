// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from session.schema.json by scripts/integrations/generate-session-contract.ts.
// Schema SHA-256 (LF): c4360598ca4ef8f9eb7dea33b1bfdf17f0fdc53f1737a0c73b21866a3947e34d

/// Schema-defined untrusted SessionScope data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionScope {
    /// Schema-defined member; admit the complete value before use.
    pub installation: String,
    /// Schema-defined member; admit the complete value before use.
    pub profile: String,
    /// Schema-defined member; admit the complete value before use.
    pub workspace: String,
    /// Schema-defined member; admit the complete value before use.
    pub backend: String,
}
impl SessionScope {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.installation.encode_utf16().count() <= 32
            && self.profile.encode_utf16().count() <= 64
            && self.workspace.encode_utf16().count() <= 36
            && self.backend.encode_utf16().count() <= 36
    }
}

/// Schema-defined untrusted SessionSource data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionSource {
    /// Schema-defined member; admit the complete value before use.
    pub id: String,
    /// Schema-defined member; admit the complete value before use.
    pub node_id: String,
    /// Schema-defined member; admit the complete value before use.
    pub execution_path: String,
    /// Schema-defined member; admit the complete value before use.
    pub label: String,
    /// Schema-defined member; admit the complete value before use.
    pub graph_revision: u64,
}
impl SessionSource {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.id.encode_utf16().count() <= 36
            && self.node_id.encode_utf16().count() <= 256
            && self.execution_path.encode_utf16().count() <= 4096
            && self.label.encode_utf16().count() <= 4096
            && (0..=9007199254740991).contains(&self.graph_revision)
    }
}

/// Schema-defined untrusted SessionScene data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionScene {
    /// Schema-defined member; admit the complete value before use.
    pub group_id: String,
    /// Schema-defined member; admit the complete value before use.
    pub scene_id: String,
    /// Schema-defined member; admit the complete value before use.
    pub order: u64,
    /// Schema-defined member; admit the complete value before use.
    pub count: u64,
    /// Schema-defined member; admit the complete value before use.
    pub title: String,
}
impl SessionScene {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.group_id.encode_utf16().count() <= 36
            && self.scene_id.encode_utf16().count() <= 36
            && (0..=8191).contains(&self.order)
            && (1..=8192).contains(&self.count)
            && self.title.encode_utf16().count() <= 4096
    }
}

/// Schema-defined untrusted SessionFile data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionFile {
    /// Schema-defined member; admit the complete value before use.
    pub storage: String,
    /// Schema-defined member; admit the complete value before use.
    pub filename: String,
    /// Schema-defined member; admit the complete value before use.
    pub subfolder: String,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub managed_artifact: Option<String>,
}
impl SessionFile {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        matches!(self.storage.as_str(), "output" | "input" | "temp")
            && self.filename.encode_utf16().count() <= 255
            && self.subfolder.encode_utf16().count() <= 4096
            && valid_optional_text(self.managed_artifact.as_deref(), 256)
    }
}

/// Schema-defined untrusted SessionAsset data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionAsset {
    /// Schema-defined member; admit the complete value before use.
    pub version: u64,
    /// Schema-defined member; admit the complete value before use.
    pub digest: String,
    /// Schema-defined member; admit the complete value before use.
    pub bytes: u64,
    /// Schema-defined member; admit the complete value before use.
    pub extension: String,
    /// Schema-defined member; admit the complete value before use.
    pub media: String,
}
impl SessionAsset {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.version == 1
            && self.digest.encode_utf16().count() <= 64
            && (1..=8589934592).contains(&self.bytes)
            && valid_session_asset_extension(&self.extension)
            && matches!(self.media.as_str(), "image" | "video")
    }
}

/// Schema-defined untrusted SessionOutput data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionOutput {
    /// Schema-defined member; admit the complete value before use.
    pub id: String,
    /// Schema-defined member; admit the complete value before use.
    pub entry_id: String,
    /// Schema-defined member; admit the complete value before use.
    pub backend: String,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub prompt_id: Option<String>,
    /// Schema-defined member; admit the complete value before use.
    pub run_id: String,
    /// Schema-defined member; admit the complete value before use.
    pub run_order: u64,
    /// Schema-defined member; admit the complete value before use.
    pub source_id: String,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub scene: Option<SessionScene>,
    /// Schema-defined member; admit the complete value before use.
    pub list_index: u64,
    /// Schema-defined member; admit the complete value before use.
    pub batch_index: u64,
    /// Schema-defined member; admit the complete value before use.
    pub image_index: u64,
    /// Schema-defined member; admit the complete value before use.
    pub file: SessionFile,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub asset_digest: Option<String>,
    /// Schema-defined member; admit the complete value before use.
    pub digest: String,
    /// Schema-defined member; admit the complete value before use.
    pub bytes: u64,
    /// Schema-defined member; admit the complete value before use.
    pub width: u64,
    /// Schema-defined member; admit the complete value before use.
    pub height: u64,
    /// Schema-defined member; admit the complete value before use.
    pub media: String,
    /// Schema-defined member; admit the complete value before use.
    pub mime: String,
    /// Schema-defined member; admit the complete value before use.
    pub metadata_embedded: bool,
}
impl SessionOutput {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.id.encode_utf16().count() <= 36
            && self.entry_id.encode_utf16().count() <= 36
            && self.backend.encode_utf16().count() <= 36
            && valid_optional_text(self.prompt_id.as_deref(), 256)
            && self.run_id.encode_utf16().count() <= 36
            && (0..=9007199254740991).contains(&self.run_order)
            && self.source_id.encode_utf16().count() <= 36
            && self.scene.as_ref().is_none_or(SessionScene::valid)
            && (0..=8191).contains(&self.list_index)
            && (0..=8191).contains(&self.batch_index)
            && (0..=8191).contains(&self.image_index)
            && self.file.valid()
            && valid_optional_text(self.asset_digest.as_deref(), 64)
            && self.digest.encode_utf16().count() <= 64
            && (1..=8589934592).contains(&self.bytes)
            && (1..=131072).contains(&self.width)
            && (1..=131072).contains(&self.height)
            && matches!(self.media.as_str(), "image" | "video")
            && self.mime.encode_utf16().count() <= 128
    }
}

/// Schema-defined untrusted SessionSelection data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionSelection {
    /// Schema-defined member; admit the complete value before use.
    pub output_id: String,
    /// Schema-defined member; admit the complete value before use.
    pub source_id: String,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub scene_id: Option<String>,
    /// Schema-defined member; admit the complete value before use.
    pub batch_index: u64,
    /// Schema-defined member; admit the complete value before use.
    pub list_index: u64,
}
impl SessionSelection {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.output_id.encode_utf16().count() <= 36
            && self.source_id.encode_utf16().count() <= 36
            && valid_optional_text(self.scene_id.as_deref(), 36)
            && (0..=8191).contains(&self.batch_index)
            && (0..=8191).contains(&self.list_index)
    }
}

/// Schema-defined untrusted SessionCamera data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionCamera {
    /// Schema-defined member; admit the complete value before use.
    pub output_id: String,
    /// Schema-defined member; admit the complete value before use.
    pub inspection: SessionInspection,
}
impl SessionCamera {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.output_id.encode_utf16().count() <= 36 && self.inspection.valid()
    }
}

/// Schema-defined untrusted SessionComparison data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionComparison {
    /// Schema-defined member; admit the complete value before use.
    pub enabled: bool,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub selection: Option<SessionSelection>,
    /// Schema-defined member; admit the complete value before use.
    pub orientation: String,
    /// Schema-defined member; admit the complete value before use.
    pub split: f64,
}
impl SessionComparison {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.selection.as_ref().is_none_or(SessionSelection::valid)
            && matches!(self.orientation.as_str(), "horizontal" | "vertical")
            && (0.0..=1.0).contains(&self.split)
    }
}

/// Schema-defined untrusted SessionView data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionView {
    /// Schema-defined member; admit the complete value before use.
    pub canvas_enabled: bool,
    /// Schema-defined member; admit the complete value before use.
    pub masonry_enabled: bool,
    /// Schema-defined member; admit the complete value before use.
    pub editor_width: u64,
    /// Schema-defined member; admit the complete value before use.
    pub route: String,
    /// Schema-defined member; admit the complete value before use.
    pub follow: String,
    /// Schema-defined member; admit the complete value before use.
    pub mode: String,
    /// Schema-defined member; admit the complete value before use.
    pub linked: bool,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub selection: Option<SessionSelection>,
    /// Schema-defined member; admit the complete value before use.
    pub comparison: SessionComparison,
    /// Schema-defined member; admit the complete value before use.
    pub cameras: Vec<SessionCamera>,
    /// Schema-defined member; admit the complete value before use.
    pub navigation: SessionNavigation,
    /// Schema-defined member; admit the complete value before use.
    pub source_preview: bool,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub shared_inspection: Option<SessionInspection>,
}
impl SessionView {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        (0..=16384).contains(&self.editor_width)
            && matches!(self.route.as_str(), "input" | "output")
            && matches!(self.follow.as_str(), "auto" | "manual")
            && matches!(self.mode.as_str(), "image" | "overview")
            && self.selection.as_ref().is_none_or(SessionSelection::valid)
            && self.comparison.valid()
            && self.cameras.len() <= 8192
            && self.cameras.iter().all(SessionCamera::valid)
            && self.navigation.valid()
            && self.shared_inspection.as_ref().is_none_or(SessionInspection::valid)
    }
}

/// Schema-defined untrusted SessionGraphView data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionGraphView {
    /// Schema-defined member; admit the complete value before use.
    pub x: f64,
    /// Schema-defined member; admit the complete value before use.
    pub y: f64,
    /// Schema-defined member; admit the complete value before use.
    pub scale: f64,
}
impl SessionGraphView {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        (-1000000000.0..=1000000000.0).contains(&self.x)
            && (-1000000000.0..=1000000000.0).contains(&self.y)
            && (0.000001..=1000000.0).contains(&self.scale)
    }
}

/// Schema-defined untrusted SessionDocument data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionDocument {
    /// Schema-defined member; admit the complete value before use.
    pub graph_json: String,
    /// Schema-defined member; admit the complete value before use.
    pub graph_revision: u64,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub path: Option<String>,
    /// Schema-defined member; admit the complete value before use.
    pub name: String,
    /// Schema-defined member; admit the complete value before use.
    pub temporary: bool,
    /// Schema-defined member; admit the complete value before use.
    pub dirty: bool,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub source_identity: Option<String>,
    /// Schema-defined member; admit the complete value before use.
    pub view: SessionGraphView,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub baseline_graph_json: Option<String>,
}
impl SessionDocument {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.graph_json.encode_utf16().count() <= 33554432
            && (0..=9007199254740991).contains(&self.graph_revision)
            && valid_optional_text(self.path.as_deref(), 4096)
            && self.name.encode_utf16().count() <= 4096
            && valid_optional_text(self.source_identity.as_deref(), 4096)
            && self.view.valid()
            && valid_optional_text(self.baseline_graph_json.as_deref(), 33554432)
    }
}

/// Schema-defined untrusted SessionInput data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionInput {
    /// Schema-defined member; admit the complete value before use.
    pub document_id: String,
    /// Schema-defined member; admit the complete value before use.
    pub generation: u64,
    /// Schema-defined member; admit the complete value before use.
    pub signature: String,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub surface: Option<String>,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub mask: Option<String>,
}
impl SessionInput {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.document_id.encode_utf16().count() <= 256
            && (0..=9007199254740991).contains(&self.generation)
            && self.signature.encode_utf16().count() <= 64
            && valid_optional_text(self.surface.as_deref(), 256)
            && valid_optional_text(self.mask.as_deref(), 256)
    }
}

/// Schema-defined untrusted SessionSubmission data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionSubmission {
    /// Schema-defined member; admit the complete value before use.
    pub entry_id: String,
    /// Schema-defined member; admit the complete value before use.
    pub backend: String,
    /// Schema-defined member; admit the complete value before use.
    pub prompt_id: String,
    /// Schema-defined member; admit the complete value before use.
    pub run_id: String,
    /// Schema-defined member; admit the complete value before use.
    pub run_order: u64,
    /// Schema-defined member; admit the complete value before use.
    pub graph_revision: u64,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub scene: Option<SessionScene>,
    /// Schema-defined member; admit the complete value before use.
    pub sources: Vec<SessionSource>,
}
impl SessionSubmission {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.entry_id.encode_utf16().count() <= 36
            && self.backend.encode_utf16().count() <= 36
            && self.prompt_id.encode_utf16().count() <= 256
            && self.run_id.encode_utf16().count() <= 36
            && (0..=9007199254740991).contains(&self.run_order)
            && (0..=9007199254740991).contains(&self.graph_revision)
            && self.scene.as_ref().is_none_or(SessionScene::valid)
            && !self.sources.is_empty()
            && self.sources.len() <= 8192
            && self.sources.iter().all(SessionSource::valid)
    }
}

/// Schema-defined untrusted SessionEntry data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionEntry {
    /// Schema-defined member; admit the complete value before use.
    pub id: String,
    /// Schema-defined member; admit the complete value before use.
    pub document: SessionDocument,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub input: Option<SessionInput>,
    /// Schema-defined member; admit the complete value before use.
    pub view: SessionView,
    /// Schema-defined member; admit the complete value before use.
    pub source_revision: u64,
    /// Schema-defined member; admit the complete value before use.
    pub navigation_revision: u64,
    /// Schema-defined member; admit the complete value before use.
    pub sources: Vec<SessionSource>,
    /// Schema-defined member; admit the complete value before use.
    pub outputs: Vec<SessionOutput>,
    /// Schema-defined member; admit the complete value before use.
    pub submissions: Vec<SessionSubmission>,
}
impl SessionEntry {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.id.encode_utf16().count() <= 36
            && self.document.valid()
            && self.input.as_ref().is_none_or(SessionInput::valid)
            && self.view.valid()
            && (0..=9007199254740991).contains(&self.source_revision)
            && (0..=9007199254740991).contains(&self.navigation_revision)
            && self.sources.len() <= 8192
            && self.sources.iter().all(SessionSource::valid)
            && self.outputs.len() <= 8192
            && self.outputs.iter().all(SessionOutput::valid)
            && self.submissions.len() <= 8192
            && self.submissions.iter().all(SessionSubmission::valid)
    }
}

/// Schema-defined untrusted SessionRect data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionRect {
    /// Schema-defined member; admit the complete value before use.
    pub x: f64,
    /// Schema-defined member; admit the complete value before use.
    pub y: f64,
    /// Schema-defined member; admit the complete value before use.
    pub width: f64,
    /// Schema-defined member; admit the complete value before use.
    pub height: f64,
}
impl SessionRect {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        (-1000000.0..=1000000.0).contains(&self.x)
            && (-1000000.0..=1000000.0).contains(&self.y)
            && (1.0..=1000000.0).contains(&self.width)
            && (1.0..=1000000.0).contains(&self.height)
    }
}

/// Schema-defined untrusted SessionPlacement data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionPlacement {
    /// Schema-defined member; admit the complete value before use.
    pub version: u64,
    /// Schema-defined member; admit the complete value before use.
    pub bounds: SessionRect,
    /// Schema-defined member; admit the complete value before use.
    pub maximized: bool,
    /// Schema-defined member; admit the complete value before use.
    pub monitor: String,
    /// Schema-defined member; admit the complete value before use.
    pub work_area: SessionRect,
    /// Schema-defined member; admit the complete value before use.
    pub dpi: f64,
}
impl SessionPlacement {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.version == 1
            && self.bounds.valid()
            && self.monitor.encode_utf16().count() <= 256
            && self.work_area.valid()
            && (24.0..=960.0).contains(&self.dpi)
    }
}

/// Schema-defined untrusted SessionCheckpoint data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionCheckpoint {
    /// Schema-defined member; admit the complete value before use.
    pub version: u64,
    /// Schema-defined member; admit the complete value before use.
    pub id: String,
    /// Schema-defined member; admit the complete value before use.
    pub revision: u64,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub previous_id: Option<String>,
    /// Schema-defined member; admit the complete value before use.
    pub captured_at: u64,
    /// Schema-defined member; admit the complete value before use.
    pub scope: SessionScope,
    /// Schema-defined member; admit the complete value before use.
    pub producer: String,
    /// Schema-defined member; admit the complete value before use.
    pub frontend: String,
    /// Schema-defined member; admit the complete value before use.
    pub sugarcubes: String,
    /// Schema-defined member; admit the complete value before use.
    pub entries: Vec<SessionEntry>,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub active_entry: Option<String>,
    /// Schema-defined member; admit the complete value before use.
    pub route: String,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub placement: Option<SessionPlacement>,
    /// Schema-defined member; admit the complete value before use.
    pub assets: Vec<SessionAsset>,
}
impl SessionCheckpoint {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.version == 1
            && self.id.encode_utf16().count() <= 36
            && (0..=9007199254740991).contains(&self.revision)
            && valid_optional_text(self.previous_id.as_deref(), 36)
            && (0..=9007199254740991).contains(&self.captured_at)
            && self.scope.valid()
            && self.producer.encode_utf16().count() <= 256
            && self.frontend.encode_utf16().count() <= 256
            && self.sugarcubes.encode_utf16().count() <= 256
            && self.entries.len() <= 256
            && self.entries.iter().all(SessionEntry::valid)
            && valid_optional_text(self.active_entry.as_deref(), 36)
            && matches!(self.route.as_str(), "workflow" | "settings")
            && self.placement.as_ref().is_none_or(SessionPlacement::valid)
            && self.assets.len() <= 65536
            && self.assets.iter().all(SessionAsset::valid)
    }
}

/// Schema-defined untrusted SessionBinding data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionBinding {
    /// Schema-defined member; admit the complete value before use.
    pub entry_id: String,
    /// Schema-defined member; admit the complete value before use.
    pub runtime_session: String,
    /// Schema-defined member; admit the complete value before use.
    pub generation: u64,
    /// Schema-defined member; admit the complete value before use.
    pub graph_revision: u64,
    /// Schema-defined member; admit the complete value before use.
    pub source_revision: u64,
    /// Schema-defined member; admit the complete value before use.
    pub navigation_revision: u64,
}
impl SessionBinding {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.entry_id.encode_utf16().count() <= 36
            && self.runtime_session.encode_utf16().count() <= 256
            && (0..=9007199254740991).contains(&self.generation)
            && (0..=9007199254740991).contains(&self.graph_revision)
            && (0..=9007199254740991).contains(&self.source_revision)
            && (0..=9007199254740991).contains(&self.navigation_revision)
    }
}

/// Schema-defined untrusted SessionFence data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionFence {
    /// Schema-defined member; admit the complete value before use.
    pub webview: String,
    /// Schema-defined member; admit the complete value before use.
    pub revision: u64,
    /// Schema-defined member; admit the complete value before use.
    pub bindings: Vec<SessionBinding>,
}
impl SessionFence {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.webview.encode_utf16().count() <= 36
            && (0..=9007199254740991).contains(&self.revision)
            && self.bindings.len() <= 256
            && self.bindings.iter().all(SessionBinding::valid)
    }
}

/// Schema-defined untrusted SessionCapabilities data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionCapabilities {
    /// Schema-defined member; admit the complete value before use.
    pub capture: bool,
    /// Schema-defined member; admit the complete value before use.
    pub materialize: bool,
    /// Schema-defined member; admit the complete value before use.
    pub chunk_transfer: bool,
    /// Schema-defined member; admit the complete value before use.
    pub artifact_resolution: bool,
}
impl SessionCapabilities {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        true
    }
}

/// Schema-defined untrusted SessionTransfer data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionTransfer {
    /// Schema-defined member; admit the complete value before use.
    pub id: String,
    /// Schema-defined member; admit the complete value before use.
    pub checkpoint_id: String,
    /// Schema-defined member; admit the complete value before use.
    pub revision: u64,
    /// Schema-defined member; admit the complete value before use.
    pub bytes: u64,
    /// Schema-defined member; admit the complete value before use.
    pub chunks: u64,
    /// Schema-defined member; admit the complete value before use.
    pub digest: String,
    /// Schema-defined member; admit the complete value before use.
    pub direction: String,
}
impl SessionTransfer {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.id.encode_utf16().count() <= 36
            && self.checkpoint_id.encode_utf16().count() <= 36
            && (0..=9007199254740991).contains(&self.revision)
            && (1..=134217728).contains(&self.bytes)
            && (1..=8192).contains(&self.chunks)
            && self.digest.encode_utf16().count() <= 64
            && matches!(self.direction.as_str(), "capture" | "materialize")
    }
}

/// Schema-defined untrusted SessionFailure data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionFailure {
    /// Schema-defined member; admit the complete value before use.
    pub code: String,
    /// Schema-defined member; admit the complete value before use.
    pub retryable: bool,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub entry_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub output_id: Option<String>,
}
impl SessionFailure {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        valid_session_failure_code(&self.code)
            && valid_optional_text(self.entry_id.as_deref(), 36)
            && valid_optional_text(self.output_id.as_deref(), 36)
    }
}

/// Schema-defined untrusted SessionOperation data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", deny_unknown_fields, rename_all_fields = "camelCase")]
pub enum SessionOperation {
    /// Schema-defined operation; lifecycle admission precedes effects.
    #[serde(rename = "ready")]
    Ready {
        /// Correlation or payload fact; grants no authority.
        capabilities: SessionCapabilities,
        /// Correlation or payload fact; grants no authority.
        committed: bool,
        /// Correlation or payload fact; grants no authority.
        workflow_persistence: bool,
        /// Correlation or payload fact; grants no authority.
        fence: SessionFence,
    },
    /// Schema-defined operation; lifecycle admission precedes effects.
    #[serde(rename = "capture")]
    Capture {
        /// Correlation or payload fact; grants no authority.
        fence: SessionFence,
        /// Correlation or payload fact; grants no authority.
        checkpoint_id: String,
        /// Correlation or payload fact; grants no authority.
        revision: u64,
    },
    /// Schema-defined operation; lifecycle admission precedes effects.
    #[serde(rename = "materialize")]
    Materialize {
        /// Correlation or payload fact; grants no authority.
        fence: SessionFence,
        /// Correlation or payload fact; grants no authority.
        transfer_id: String,
    },
    /// Schema-defined operation; lifecycle admission precedes effects.
    #[serde(rename = "begin")]
    Begin {
        /// Correlation or payload fact; grants no authority.
        transfer: SessionTransfer,
    },
    /// Schema-defined operation; lifecycle admission precedes effects.
    #[serde(rename = "chunk")]
    Chunk {
        /// Correlation or payload fact; grants no authority.
        transfer_id: String,
        /// Correlation or payload fact; grants no authority.
        index: u64,
        /// Correlation or payload fact; grants no authority.
        text: String,
    },
    /// Schema-defined operation; lifecycle admission precedes effects.
    #[serde(rename = "commit")]
    Commit {
        /// Correlation or payload fact; grants no authority.
        transfer_id: String,
    },
    /// Schema-defined operation; lifecycle admission precedes effects.
    #[serde(rename = "cancel")]
    Cancel {
        /// Correlation or payload fact; grants no authority.
        target_request_id: String,
    },
    /// Schema-defined operation; lifecycle admission precedes effects.
    #[serde(rename = "acknowledged")]
    Acknowledged {
        /// Correlation or payload fact; grants no authority.
        checkpoint_id: String,
        /// Correlation or payload fact; grants no authority.
        revision: u64,
        /// Correlation or payload fact; grants no authority.
        phase: String,
        /// Correlation or payload fact; grants no authority.
        bindings: Vec<SessionBinding>,
    },
    /// Schema-defined operation; lifecycle admission precedes effects.
    #[serde(rename = "error")]
    Error {
        /// Correlation or payload fact; grants no authority.
        failure: SessionFailure,
    },
    /// Schema-defined operation; lifecycle admission precedes effects.
    #[serde(rename = "resolve")]
    Resolve {
        /// Correlation or payload fact; grants no authority.
        output: Box<SessionOutput>,
    },
    /// Schema-defined operation; lifecycle admission precedes effects.
    #[serde(rename = "resolved")]
    Resolved {
        /// Correlation or payload fact; grants no authority.
        output_id: String,
        /// Correlation or payload fact; grants no authority.
        digest: String,
        /// Correlation or payload fact; grants no authority.
        bytes: u64,
        /// Correlation or payload fact; grants no authority.
        file: SessionFile,
    },
}
impl SessionOperation {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        match self {
            Self::Ready { capabilities, committed: _, workflow_persistence: _, fence } => {
                capabilities.valid() && fence.valid()
            }
            Self::Capture { fence, checkpoint_id, revision } => {
                fence.valid()
                    && checkpoint_id.encode_utf16().count() <= 36
                    && (0..=9007199254740991).contains(revision)
            }
            Self::Materialize { fence, transfer_id } => {
                fence.valid() && transfer_id.encode_utf16().count() <= 36
            }
            Self::Begin { transfer } => transfer.valid(),
            Self::Chunk { transfer_id, index, text } => {
                transfer_id.encode_utf16().count() <= 36
                    && (0..=8191).contains(index)
                    && text.encode_utf16().count() <= 65536
            }
            Self::Commit { transfer_id } => transfer_id.encode_utf16().count() <= 36,
            Self::Cancel { target_request_id } => target_request_id.encode_utf16().count() <= 36,
            Self::Acknowledged { checkpoint_id, revision, phase, bindings } => {
                checkpoint_id.encode_utf16().count() <= 36
                    && (0..=9007199254740991).contains(revision)
                    && valid_session_operation_acknowledged_phase(phase)
                    && bindings.len() <= 256
                    && bindings.iter().all(SessionBinding::valid)
            }
            Self::Error { failure } => failure.valid(),
            Self::Resolve { output } => output.valid(),
            Self::Resolved { output_id, digest, bytes, file } => {
                output_id.encode_utf16().count() <= 36
                    && digest.encode_utf16().count() <= 64
                    && (1..=8589934592).contains(bytes)
                    && file.valid()
            }
        }
    }
}

/// Schema-defined untrusted SessionMessage data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionMessage {
    /// Schema-defined member; admit the complete value before use.
    pub protocol: String,
    /// Schema-defined member; admit the complete value before use.
    pub version: u64,
    /// Schema-defined member; admit the complete value before use.
    pub request_id: String,
    /// Schema-defined member; admit the complete value before use.
    pub scope: SessionScope,
    /// Schema-defined member; admit the complete value before use.
    pub origin: String,
    /// Schema-defined member; admit the complete value before use.
    pub operation: SessionOperation,
}
impl SessionMessage {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        self.protocol == "sugarcubes.desktop-session"
            && self.version == 1
            && self.request_id.encode_utf16().count() <= 36
            && self.scope.valid()
            && self.origin.encode_utf16().count() <= 2048
            && self.operation.valid()
    }
}

/// Schema-defined untrusted SessionNavigation data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionNavigation {
    /// Schema-defined member; admit the complete value before use.
    pub scene_overview: bool,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub scene_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub source_id: Option<String>,
    /// Schema-defined member; admit the complete value before use.
    pub batch_index: u64,
    /// Schema-defined member; admit the complete value before use.
    pub batch_overview: bool,
}
impl SessionNavigation {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        valid_optional_text(self.scene_id.as_deref(), 36)
            && valid_optional_text(self.source_id.as_deref(), 36)
            && (0..=8191).contains(&self.batch_index)
    }
}

/// Schema-defined untrusted SessionInspection data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionInspection {
    /// Schema-defined member; admit the complete value before use.
    pub mode: String,
    /// Schema-defined member; admit the complete value before use.
    pub center_x: f64,
    /// Schema-defined member; admit the complete value before use.
    pub center_y: f64,
    /// Schema-defined member; admit the complete value before use.
    pub span_x: f64,
    /// Schema-defined member; admit the complete value before use.
    pub span_y: f64,
}
impl SessionInspection {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        matches!(self.mode.as_str(), "fit" | "native" | "custom")
            && (-1000000.0..=1000000.0).contains(&self.center_x)
            && (-1000000.0..=1000000.0).contains(&self.center_y)
            && (1e-12..=1000000000000.0).contains(&self.span_x)
            && (1e-12..=1000000000000.0).contains(&self.span_y)
    }
}

fn valid_optional_text(value: Option<&str>, maximum: usize) -> bool {
    value.is_none_or(|value| value.encode_utf16().count() <= maximum)
}

fn valid_session_asset_extension(value: &str) -> bool {
    matches!(value, "png" | "jpg" | "webp" | "gif" | "mp4" | "webm")
}

fn valid_session_failure_code(value: &str) -> bool {
    matches!(
        value,
        "unsupported"
            | "invalid"
            | "too-large"
            | "stale"
            | "cancelled"
            | "untrusted-origin"
            | "unavailable"
            | "identity-mismatch"
            | "quota"
            | "storage"
            | "migration-required"
            | "opted-out"
            | "not-committed"
    )
}

fn valid_session_operation_acknowledged_phase(value: &str) -> bool {
    matches!(value, "captured" | "materialized" | "durable" | "restored" | "cancelled")
}

/// Preserve a required nullable field instead of defaulting a missing member to null.
fn deserialize_required_option<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    serde::Deserialize::deserialize(deserializer)
}
