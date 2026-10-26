// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Typed interaction and lifecycle boundaries for built-in and host-registered canvas tools.
use super::*;

/// Positive tool identity scoped to one registry; identities are never inferred from display text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToolId(u32);
impl ToolId {
    /// The always-available built-in navigation tool.
    pub const PAN_ZOOM: Self = Self(1);
    /// Validate an application-assigned identity.
    /// # Errors
    /// Rejects zero, which cannot be selected or registered.
    pub fn new(value: u32) -> Result<Self, CanvasError> {
        if value == 0 {
            return Err(CanvasError::Invalid("Zero tool identity"));
        }
        Ok(Self(value))
    }
    /// Original identity value.
    pub const fn get(self) -> u32 {
        self.0
    }
}
/// Pointer button identity, independent of native event representations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolButton {
    /// Primary editing/panning button.
    Primary,
    /// Additional direct-panning button.
    Middle,
    /// Secondary/context button.
    Secondary,
}
/// Why the effective tool must release all transient gesture state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolCancelReason {
    /// Persistent selection changed.
    SelectionChanged,
    /// Space temporarily handed interaction to navigation.
    TemporaryNavigation,
    /// Native focus or pointer capture was lost.
    FocusLost,
    /// An authored document or source binding was replaced.
    DocumentChanged,
    /// Physical client geometry changed, invalidating the gesture's coordinate basis.
    SurfaceChanged,
    /// Navigation permission was revoked.
    Locked,
    /// The surface is retiring.
    Shutdown,
}
/// Tool-neutral physical observations. Wheel steps are normalized hardware notches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToolInput {
    /// Physical client pointer position.
    Pointer([f64; 2]),
    /// Pointer button transition at the most recent position.
    Button {
        /// Changed button.
        button: ToolButton,
        /// Whether it is pressed.
        pressed: bool,
    },
    /// A wheel observation with its explicit or implicit gesture lifetime.
    Wheel {
        /// Signed hardware notches, including fractional values.
        steps: f64,
        /// Native gesture phase.
        phase: ScrollPhase,
    },
    /// A native-recognized double click at this physical point.
    DoubleClick([f64; 2]),
    /// Request centered Fit.
    Fit,
    /// Request anchored native scale.
    OneToOne,
}
/// Read-only context for activation and cursor resolution; no scheduling or native authority.
pub struct ToolView<'a> {
    /// Authoritative current navigation state.
    pub navigation: &'a Navigation,
    /// Current immutable authored metadata, when a document is available.
    pub document: Option<DocumentSnapshot>,
}
/// One reusable tool lifecycle. Implementations keep only transient interaction state.
/// Document mutation goes through ToolContext's owned edit session. Native handles, execution,
/// caches, rendering and durable history remain with their existing owners.
pub trait CanvasTool: Send {
    /// Validate activation from a read-only snapshot before the previous tool is cancelled.
    /// # Errors
    /// An activation rejection leaves selection, capture and the preceding gesture intact.
    fn activate(&mut self, _view: &ToolView<'_>) -> Result<(), CanvasError> {
        Ok(())
    }
    /// Interpret an observation using the current admitted gesture boundary.
    /// # Errors
    /// Preserve input or document causes. Uncommitted edits remain independently cancellable.
    fn input(&mut self, input: ToolInput, context: &mut ToolContext<'_>)
    -> Result<(), CanvasError>;
    /// Release transient tool state without I/O. The manager independently cancels capture/edits.
    fn cancel(&mut self, reason: ToolCancelReason);
    /// Resolve feedback from the current shared state without allocating a native cursor.
    fn cursor(&self, view: &ToolView<'_>) -> CursorIntent;
    /// Whether Space belongs to this tool's own text/interaction protocol.
    fn captures_space(&self) -> bool {
        false
    }
}
