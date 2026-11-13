// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode the editing state the host reports in `edit.reply` and `edit.changed`.

use super::{
    CanvasBrushOptions, CanvasDocumentId, CanvasEditMode, CanvasLayerId, CanvasNativeError,
    CanvasNativeNotificationKind, CanvasNativeNotificationPayload, CanvasToolId,
    MAX_CANVAS_EDIT_REPLY_BYTES, ReplyEnvelope, wire,
};
use crate::panel::PanelSession;

/// Whether one offered tool can be used right now.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CanvasEditTool {
    /// The tool.
    pub id: CanvasToolId,
    /// Whether it can be selected now.
    pub available: bool,
}

/// Selection bounds in surface CSS pixels relative to the surface geometry origin.
///
/// They follow pan and zoom so the page can place the selection toolbar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasEditSelectionBounds {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

impl CanvasEditSelectionBounds {
    /// Largest magnitude of any coordinate or extent, in CSS pixels.
    pub const MAX_CSS_PIXELS: f64 = 1_000_000.0;

    /// Adopt bounds with a positive extent within ±1,000,000 CSS pixels.
    ///
    /// # Errors
    /// Rejects non-finite values, an origin outside the range or a non-positive extent.
    pub fn new(left: f64, top: f64, width: f64, height: f64) -> Result<Self, CanvasNativeError> {
        let origin = -Self::MAX_CSS_PIXELS..=Self::MAX_CSS_PIXELS;
        let extent = |value: f64| value > 0.0 && value <= Self::MAX_CSS_PIXELS;
        if !origin.contains(&left) || !origin.contains(&top) || !extent(width) || !extent(height) {
            return Err(CanvasNativeError::Invalid);
        }
        Ok(Self { left, top, width, height })
    }

    /// Return the left edge, top edge, width and height in CSS pixels.
    pub const fn rect(self) -> (f64, f64, f64, f64) {
        (self.left, self.top, self.width, self.height)
    }
}

/// The kinds of edit session the toolbar can show.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasEditSessionKind {
    /// Moving content.
    Move,
    /// Transforming floating content.
    Transform,
    /// Editing shared mask edges.
    SharedEdges,
    /// Previewing a coverage modification.
    Modify,
}

impl CanvasEditSessionKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::Transform => "transform",
            Self::SharedEdges => "shared-edges",
            Self::Modify => "modify",
        }
    }
}

/// The active edit session and its own undo and redo availability.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CanvasEditSession {
    /// The session kind.
    pub kind: CanvasEditSessionKind,
    /// Whether session undo is available.
    pub undo: bool,
    /// Whether session redo is available.
    pub redo: bool,
}

/// The complete editing state; every successful command and every change reports all of it.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasEditState {
    /// Monotonic revision, at most 2^53 − 1 so JavaScript reads it exactly.
    pub revision: u64,
    /// The editable input document shown, if any.
    pub document: Option<CanvasDocumentId>,
    /// The active mask layer, if any.
    pub active_mask: Option<CanvasLayerId>,
    /// The active tool.
    pub tool: CanvasToolId,
    /// Every offered tool, at most 12 entries.
    pub tools: Vec<CanvasEditTool>,
    /// Brush options.
    pub brush: CanvasBrushOptions,
    /// Eraser options.
    pub eraser: CanvasBrushOptions,
    /// Selection bounds, if a selection exists.
    pub selection: Option<CanvasEditSelectionBounds>,
    /// Whether document undo is available.
    pub undo: bool,
    /// Whether document redo is available.
    pub redo: bool,
    /// The active edit session, if any.
    pub session: Option<CanvasEditSession>,
    /// The panel mode.
    pub mode: CanvasEditMode,
}

impl CanvasEditState {
    /// Encode `edit.changed` for one panel session.
    ///
    /// The host sends this whenever the state changes, coalesced to at most one per frame.
    /// # Errors
    /// Rejects state outside the shared bounds and preserves serialization failures.
    pub fn changed(
        &self,
        session: &PanelSession,
    ) -> Result<CanvasNativeNotificationPayload, CanvasNativeError> {
        let ReplyEnvelope { protocol, version, session: name, request_id } =
            ReplyEnvelope::notification(session);
        let notification = wire::CanvasEditChanged {
            protocol,
            version,
            session: name,
            request_id,
            r#type: "edit.changed".into(),
            state: self.to_wire(),
        };
        CanvasNativeNotificationPayload::new(
            session,
            CanvasNativeNotificationKind::EditChanged,
            &notification,
            notification.valid(),
            MAX_CANVAS_EDIT_REPLY_BYTES,
        )
    }

    pub(super) fn to_wire(&self) -> wire::CanvasEditState {
        wire::CanvasEditState {
            revision: self.revision,
            document: self.document.as_ref().map(|id| id.as_str().into()),
            active_mask: self.active_mask.as_ref().map(|id| id.as_str().into()),
            tool: self.tool.to_wire(),
            tools: self
                .tools
                .iter()
                .map(|tool| wire::CanvasEditTool {
                    id: tool.id.to_wire(),
                    available: tool.available,
                })
                .collect(),
            options: wire::CanvasEditToolOptions {
                brush: self.brush.to_wire(),
                eraser: self.eraser.to_wire(),
            },
            selection: self.selection.map(|bounds| wire::CanvasEditSelection {
                bounds: wire::CanvasEditSelectionBounds {
                    left: bounds.left,
                    top: bounds.top,
                    width: bounds.width,
                    height: bounds.height,
                },
            }),
            history: wire::CanvasEditHistory { undo: self.undo, redo: self.redo },
            session: self.session.map(|session| wire::CanvasEditSession {
                kind: session.kind.as_str().into(),
                undo: session.undo,
                redo: session.redo,
            }),
            mode: self.mode.as_str().into(),
        }
    }
}
