// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Generated from session-checkpoint-v2.schema.json by scripts/integrations/generate-session-contract.ts.
// Schema SHA-256 (LF): de02284c6963bbcded949a47be307bcd2a01108890c610392940c6934a2902e0

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
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub scroll: Option<SessionScroll>,
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
            && self.scroll.as_ref().is_none_or(SessionScroll::valid)
    }
}

/// Schema-defined untrusted SessionScroll data; validation grants no host authority.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionScroll {
    #[serde(deserialize_with = "deserialize_required_option")]
    /// Schema-defined member; admit the complete value before use.
    pub cube: Option<String>,
    /// Schema-defined member; admit the complete value before use.
    pub offset: u64,
}
impl SessionScroll {
    /// Reject values outside the exact schema-defined bounds.
    pub fn valid(&self) -> bool {
        valid_optional_text(self.cube.as_deref(), 256) && (0..=10000000).contains(&self.offset)
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
        self.version == 2
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

fn valid_optional_text(value: Option<&str>, maximum: usize) -> bool {
    value.is_none_or(|value| value.encode_utf16().count() <= maximum)
}

/// Preserve a required nullable field instead of defaulting a missing member to null.
fn deserialize_required_option<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    serde::Deserialize::deserialize(deserializer)
}
