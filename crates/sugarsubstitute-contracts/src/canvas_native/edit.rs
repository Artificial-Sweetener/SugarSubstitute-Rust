// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode `edit.request` commands from the editing UI into owned tool and history intents.

use super::{
    CanvasEditState, CanvasFailure, CanvasNativeEnvelope, CanvasNativeError,
    CanvasNativeReplyPayload, MAX_CANVAS_EDIT_REPLY_BYTES, MAX_CANVAS_EDIT_REQUEST_BYTES,
    ReplyEnvelope, admit, parse, wire,
};

/// The editing tools SugarCubes can offer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasToolId {
    /// Pan and zoom the view.
    PanZoom,
    /// Move layer content or a selection.
    Move,
    /// Paint coverage.
    Brush,
    /// Erase coverage.
    Eraser,
    /// Select a rectangle.
    SelectRectangle,
    /// Select an ellipse.
    SelectEllipse,
    /// Select a freehand polygon.
    SelectLasso,
    /// Fill a rectangle of the active mask.
    MaskRectangle,
    /// Fill an ellipse of the active mask.
    MaskEllipse,
    /// Fill a freehand polygon of the active mask.
    MaskLasso,
    /// Transform floating content.
    Transform,
    /// Edit the edges shared between masks.
    SharedEdges,
}

impl CanvasToolId {
    /// Every tool, in the order SugarCubes lists them.
    pub const ALL: [Self; 12] = [
        Self::PanZoom,
        Self::Move,
        Self::Brush,
        Self::Eraser,
        Self::SelectRectangle,
        Self::SelectEllipse,
        Self::SelectLasso,
        Self::MaskRectangle,
        Self::MaskEllipse,
        Self::MaskLasso,
        Self::Transform,
        Self::SharedEdges,
    ];

    pub(super) const fn from_wire(value: wire::CanvasToolId) -> Self {
        match value {
            wire::CanvasToolId::PanZoom => Self::PanZoom,
            wire::CanvasToolId::Move => Self::Move,
            wire::CanvasToolId::Brush => Self::Brush,
            wire::CanvasToolId::Eraser => Self::Eraser,
            wire::CanvasToolId::SelectRectangle => Self::SelectRectangle,
            wire::CanvasToolId::SelectEllipse => Self::SelectEllipse,
            wire::CanvasToolId::SelectLasso => Self::SelectLasso,
            wire::CanvasToolId::MaskRectangle => Self::MaskRectangle,
            wire::CanvasToolId::MaskEllipse => Self::MaskEllipse,
            wire::CanvasToolId::MaskLasso => Self::MaskLasso,
            wire::CanvasToolId::Transform => Self::Transform,
            wire::CanvasToolId::SharedEdges => Self::SharedEdges,
        }
    }

    pub(super) const fn to_wire(self) -> wire::CanvasToolId {
        match self {
            Self::PanZoom => wire::CanvasToolId::PanZoom,
            Self::Move => wire::CanvasToolId::Move,
            Self::Brush => wire::CanvasToolId::Brush,
            Self::Eraser => wire::CanvasToolId::Eraser,
            Self::SelectRectangle => wire::CanvasToolId::SelectRectangle,
            Self::SelectEllipse => wire::CanvasToolId::SelectEllipse,
            Self::SelectLasso => wire::CanvasToolId::SelectLasso,
            Self::MaskRectangle => wire::CanvasToolId::MaskRectangle,
            Self::MaskEllipse => wire::CanvasToolId::MaskEllipse,
            Self::MaskLasso => wire::CanvasToolId::MaskLasso,
            Self::Transform => wire::CanvasToolId::Transform,
            Self::SharedEdges => wire::CanvasToolId::SharedEdges,
        }
    }
}

/// The tools whose options are brush options.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasBrushTool {
    /// [`CanvasToolId::Brush`].
    Brush,
    /// [`CanvasToolId::Eraser`].
    Eraser,
}

/// Brush or eraser options: size `1..=1000` px, hardness and opacity `0..=100` percent.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CanvasBrushOptions {
    size: u16,
    hardness: u8,
    opacity: u8,
}

impl CanvasBrushOptions {
    /// Adopt options within the shared ranges.
    ///
    /// # Errors
    /// Rejects a size outside `1..=1000` or a percentage above 100.
    pub fn new(size: u16, hardness: u8, opacity: u8) -> Result<Self, CanvasNativeError> {
        if !(1..=1000).contains(&size) || hardness > 100 || opacity > 100 {
            return Err(CanvasNativeError::Invalid);
        }
        Ok(Self { size, hardness, opacity })
    }

    /// Return the diameter in document pixels.
    pub const fn size(self) -> u16 {
        self.size
    }

    /// Return the hardness percentage.
    pub const fn hardness(self) -> u8 {
        self.hardness
    }

    /// Return the opacity percentage.
    pub const fn opacity(self) -> u8 {
        self.opacity
    }

    pub(super) fn from_wire(value: &wire::CanvasBrushOptions) -> Result<Self, CanvasNativeError> {
        let narrow = |value: u64| u8::try_from(value).map_err(|_| CanvasNativeError::Invalid);
        Self::new(
            u16::try_from(value.size).map_err(|_| CanvasNativeError::Invalid)?,
            narrow(value.hardness)?,
            narrow(value.opacity)?,
        )
    }

    pub(super) fn to_wire(self) -> wire::CanvasBrushOptions {
        wire::CanvasBrushOptions {
            size: self.size.into(),
            hardness: self.hardness.into(),
            opacity: self.opacity.into(),
        }
    }
}

/// Document undo or redo.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasHistoryAction {
    /// Undo the last document step.
    Undo,
    /// Redo the next document step.
    Redo,
}

/// Selection toolbar actions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasSelectionAction {
    /// Drop the selection.
    Deselect,
    /// Remove the selected area from the active mask.
    Clear,
}

/// What a coverage modification changes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasModifyTarget {
    /// The selection.
    Selection,
    /// The whole active mask.
    Mask,
}

/// The coverage modification operations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasModifyOperation {
    /// Grow coverage by distance.
    Expand,
    /// Shrink coverage by distance.
    Contract,
    /// Blur coverage edges.
    Feather,
}

/// The live-preview phases of one coverage modification.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasModifyPhase {
    /// Show the result without committing it.
    Preview,
    /// Commit the previewed result as one undoable step.
    Apply,
    /// Discard the preview.
    Cancel,
}

/// One live-previewed coverage modification by `pixels`, `1..=999`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CanvasCoverageModify {
    /// What changes.
    pub target: CanvasModifyTarget,
    /// How it changes.
    pub operation: CanvasModifyOperation,
    /// Distance or radius in document pixels, `1..=999`.
    pub pixels: u16,
    /// Preview, apply or cancel.
    pub phase: CanvasModifyPhase,
}

/// Whether the "Edit layer coverage" panel mode is open; canvas input is blocked while it is.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasEditMode {
    /// Ordinary editing.
    None,
    /// The coverage panel is open.
    Coverage,
}

impl CanvasEditMode {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Coverage => "coverage",
        }
    }
}

/// Transform toolbar actions; apply and cancel arrive as commit and cancel.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasTransformAction {
    /// Rotate 90° counter-clockwise.
    RotateLeft,
    /// Rotate 90° clockwise.
    RotateRight,
    /// Mirror left to right.
    FlipHorizontal,
    /// Mirror top to bottom.
    FlipVertical,
}

/// Edit-session toolbar actions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasSessionAction {
    /// Undo within the session.
    Undo,
    /// Redo within the session.
    Redo,
    /// Apply the session as one document step.
    Apply,
    /// Discard the session.
    Cancel,
}

/// One `edit.request` command. Every successful command replies with the resulting state.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasEditCommand {
    /// Read the current state.
    State,
    /// Select a tool.
    Tool(CanvasToolId),
    /// Set brush or eraser options.
    Options {
        /// Which tool's options.
        tool: CanvasBrushTool,
        /// The new options.
        options: CanvasBrushOptions,
    },
    /// Document undo or redo.
    History(CanvasHistoryAction),
    /// Enter: commit the active gesture, floating pixels, transform or session.
    Commit,
    /// Escape: cancel the active gesture, floating pixels, transform or session.
    Cancel,
    /// Arrow keys, in document pixels `-100..=100` per axis.
    Nudge {
        /// Horizontal step.
        dx: i8,
        /// Vertical step.
        dy: i8,
    },
    /// A selection toolbar action.
    Selection(CanvasSelectionAction),
    /// A live-previewed coverage modification.
    Modify(CanvasCoverageModify),
    /// Open or close the coverage panel mode.
    Mode(CanvasEditMode),
    /// A transform toolbar action.
    Transform(CanvasTransformAction),
    /// An edit-session toolbar action.
    Session(CanvasSessionAction),
    /// Temporary navigation while Space is held.
    Navigation {
        /// Whether Space is held.
        held: bool,
    },
}

impl CanvasEditCommand {
    fn from_wire(value: wire::CanvasEditCommand) -> Result<Self, CanvasNativeError> {
        use wire::CanvasEditCommand as Wire;
        let invalid = || CanvasNativeError::Invalid;
        Ok(match value {
            Wire::State {} => Self::State,
            Wire::Tool { tool } => Self::Tool(CanvasToolId::from_wire(tool)),
            Wire::Options { tool, options } => Self::Options {
                tool: match tool.as_str() {
                    "brush" => CanvasBrushTool::Brush,
                    "eraser" => CanvasBrushTool::Eraser,
                    _ => return Err(invalid()),
                },
                options: CanvasBrushOptions::from_wire(&options)?,
            },
            Wire::History { action } => Self::History(match action.as_str() {
                "undo" => CanvasHistoryAction::Undo,
                "redo" => CanvasHistoryAction::Redo,
                _ => return Err(invalid()),
            }),
            Wire::Commit {} => Self::Commit,
            Wire::Cancel {} => Self::Cancel,
            Wire::Nudge { dx, dy } => Self::Nudge {
                dx: i8::try_from(dx).map_err(|_| invalid())?,
                dy: i8::try_from(dy).map_err(|_| invalid())?,
            },
            Wire::Selection { action } => Self::Selection(match action.as_str() {
                "deselect" => CanvasSelectionAction::Deselect,
                "clear" => CanvasSelectionAction::Clear,
                _ => return Err(invalid()),
            }),
            Wire::Modify { target, operation, pixels, phase } => {
                Self::Modify(CanvasCoverageModify {
                    target: match target.as_str() {
                        "selection" => CanvasModifyTarget::Selection,
                        "mask" => CanvasModifyTarget::Mask,
                        _ => return Err(invalid()),
                    },
                    operation: match operation.as_str() {
                        "expand" => CanvasModifyOperation::Expand,
                        "contract" => CanvasModifyOperation::Contract,
                        "feather" => CanvasModifyOperation::Feather,
                        _ => return Err(invalid()),
                    },
                    pixels: u16::try_from(pixels).map_err(|_| invalid())?,
                    phase: match phase.as_str() {
                        "preview" => CanvasModifyPhase::Preview,
                        "apply" => CanvasModifyPhase::Apply,
                        "cancel" => CanvasModifyPhase::Cancel,
                        _ => return Err(invalid()),
                    },
                })
            }
            Wire::Mode { mode } => Self::Mode(match mode.as_str() {
                "none" => CanvasEditMode::None,
                "coverage" => CanvasEditMode::Coverage,
                _ => return Err(invalid()),
            }),
            Wire::Transform { action } => Self::Transform(match action.as_str() {
                "rotate-left" => CanvasTransformAction::RotateLeft,
                "rotate-right" => CanvasTransformAction::RotateRight,
                "flip-horizontal" => CanvasTransformAction::FlipHorizontal,
                "flip-vertical" => CanvasTransformAction::FlipVertical,
                _ => return Err(invalid()),
            }),
            Wire::Session { action } => Self::Session(match action.as_str() {
                "undo" => CanvasSessionAction::Undo,
                "redo" => CanvasSessionAction::Redo,
                "apply" => CanvasSessionAction::Apply,
                "cancel" => CanvasSessionAction::Cancel,
                _ => return Err(invalid()),
            }),
            Wire::Navigation { held } => Self::Navigation { held },
        })
    }
}

/// A validated `edit.request` from one panel session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasEditRequest {
    envelope: CanvasNativeEnvelope,
    command: CanvasEditCommand,
}

impl CanvasEditRequest {
    /// Decode the exact shared `edit.request`.
    ///
    /// # Errors
    /// Rejects oversized, malformed, extended, duplicate or schema-invalid messages.
    pub fn decode(bytes: &[u8]) -> Result<Self, CanvasNativeError> {
        admit(bytes, MAX_CANVAS_EDIT_REQUEST_BYTES)?;
        let request: wire::CanvasEditRequest = parse(bytes)?;
        if !request.valid() {
            return Err(CanvasNativeError::Invalid);
        }
        Ok(Self {
            command: CanvasEditCommand::from_wire(request.command)?,
            envelope: CanvasNativeEnvelope::new(request.session, request.request_id)?,
        })
    }

    /// Borrow the request's public correlation.
    pub const fn envelope(&self) -> &CanvasNativeEnvelope {
        &self.envelope
    }

    /// Return the decoded command.
    pub const fn command(&self) -> CanvasEditCommand {
        self.command
    }

    /// Encode the state that resulted from the command.
    ///
    /// # Errors
    /// Rejects state outside the shared bounds and preserves serialization failures.
    pub fn state_reply(
        &self,
        state: &CanvasEditState,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        self.reply(wire::CanvasEditResult::State { state: state.to_wire() })
    }

    /// Encode a typed failure for any edit command.
    ///
    /// # Errors
    /// Preserves serialization failures.
    pub fn failure_reply(
        &self,
        failure: &CanvasFailure,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        let wire::CanvasFailure { status, message } = failure.to_wire();
        self.reply(wire::CanvasEditResult::Failure { status, message })
    }

    fn reply(
        &self,
        result: wire::CanvasEditResult,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        let ReplyEnvelope { protocol, version, session, request_id } =
            ReplyEnvelope::of(&self.envelope);
        let reply = wire::CanvasEditReply {
            protocol,
            version,
            session,
            request_id,
            r#type: "edit.reply".into(),
            result,
        };
        self.envelope.payload(&reply, reply.valid(), MAX_CANVAS_EDIT_REPLY_BYTES)
    }
}
