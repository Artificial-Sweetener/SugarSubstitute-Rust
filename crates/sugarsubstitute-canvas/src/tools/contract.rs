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
    /// The person cancelled, such as by pressing Escape.
    Requested,
}
/// Physical device class that produced a pointer observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerKind {
    /// A mouse, touchpad or any device the platform reports as a mouse.
    Mouse,
    /// A pen or stylus, including its eraser end.
    Pen,
    /// A touch contact.
    Touch,
}
/// Keyboard modifiers the platform reported held when it delivered an observation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ToolModifiers {
    /// Either Shift key.
    pub shift: bool,
    /// Either Control key.
    pub ctrl: bool,
    /// Either Alt key.
    pub alt: bool,
}
impl ToolModifiers {
    /// No modifier held.
    pub const NONE: Self = Self { shift: false, ctrl: false, alt: false };
}
/// Contact pressure normalized to the closed unit interval; devices without pressure report
/// [`Pressure::FULL`].
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Pressure(f64);
impl Pressure {
    /// Full pressure: mouse buttons and pressure-less devices.
    pub const FULL: Self = Self(1.0);
    /// Normalize a device reading from its inclusive `0..=maximum` range. Readings above the
    /// range clamp to full pressure; a device without a range (`maximum == 0`) reports full.
    pub fn from_range(value: u32, maximum: u32) -> Self {
        if maximum == 0 {
            return Self::FULL;
        }
        Self(f64::from(value.min(maximum)) / f64::from(maximum))
    }
    /// Pressure in `[0, 1]`.
    pub const fn get(self) -> f64 {
        self.0
    }
}
/// One pointer observation with the device facts tools use to shape their gestures.
///
/// `position` is in physical pixels of the receiving coordinate space: the native host's client
/// area until the host converts it to the canvas viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerSample {
    /// Physical pixel position.
    pub position: [f64; 2],
    /// Device class that produced the observation.
    pub kind: PointerKind,
    /// Normalized contact pressure; full for mice and pressure-less devices.
    pub pressure: Pressure,
    /// Whether a pen's eraser end, inverted tip or eraser button is active.
    pub eraser: bool,
    /// Modifiers held when the platform delivered the observation.
    pub modifiers: ToolModifiers,
}
impl PointerSample {
    /// A mouse observation: full pressure and no eraser.
    pub const fn mouse(position: [f64; 2], modifiers: ToolModifiers) -> Self {
        Self {
            position,
            kind: PointerKind::Mouse,
            pressure: Pressure::FULL,
            eraser: false,
            modifiers,
        }
    }
    /// The same device facts at a position in another coordinate space.
    pub const fn at(self, position: [f64; 2]) -> Self {
        Self { position, ..self }
    }
}
/// Tool-neutral physical observations. Wheel steps are normalized hardware notches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToolInput {
    /// A pointer observation in physical viewport pixels.
    Pointer(PointerSample),
    /// Pointer button transition at the most recent pointer observation. A pen tip contact is
    /// the primary button; its pressure and eraser state arrive with the preceding sample.
    Button {
        /// Changed button.
        button: ToolButton,
        /// Whether it is pressed.
        pressed: bool,
        /// Modifiers held when the platform delivered the transition.
        modifiers: ToolModifiers,
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
    /// The layer editing tools act on, such as the active mask.
    pub target: Option<LayerId>,
}
/// A typed editing command for the selected tool, as the editing UI sends Enter, Escape, the
/// arrow keys and the transform and edit-session toolbars.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolCommand {
    /// Enter: commit floating pixels, apply a transform or an edit session.
    Commit,
    /// Escape: cancel the gesture, floating pixels, a transform or an edit session.
    Cancel,
    /// An arrow key: move content by whole document pixels, `[dx, dy]`.
    Nudge([i32; 2]),
    /// A transform toolbar action.
    Transform(TransformAction),
    /// An edit-session toolbar action.
    Session(SessionAction),
}
/// Discrete transform toolbar actions, each one edit-session checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransformAction {
    /// Turn a quarter counterclockwise about the frame centre.
    RotateLeft,
    /// Turn a quarter clockwise about the frame centre.
    RotateRight,
    /// Mirror left to right about the frame centre.
    FlipHorizontal,
    /// Mirror top to bottom about the frame centre.
    FlipVertical,
}
/// Edit-session toolbar actions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionAction {
    /// Step back one session checkpoint; never past the session's base.
    Undo,
    /// Step forward one undone session checkpoint.
    Redo,
    /// Publish the session's current result as one document undo step and close it.
    Apply,
    /// Restore the session's base and close it.
    Cancel,
}
/// What an unresolved edit session edits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionKind {
    /// Floating selected pixels moved by the Move tool.
    Move,
    /// A layer or selection under the Transform tool.
    Transform,
    /// Masks resized together along their shared edges.
    SharedEdges,
    /// A live-previewed expand, contract or feather.
    Modify,
}
/// The unresolved edit session the editing UI reports: its kind and whether its own Undo and
/// Redo can act.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionState {
    /// What the session edits.
    pub kind: SessionKind,
    /// Whether session Undo can act.
    pub undo: bool,
    /// Whether session Redo can act.
    pub redo: bool,
}
/// Settings the host chooses for a registered tool, delivered through
/// [`ToolManager::configure`] and observed through [`ToolManager::options`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToolOptions {
    /// Brush and Eraser size, hardness and opacity.
    Brush(BrushOptions),
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
    /// Settle the gesture before temporary Space navigation takes over, for example by
    /// committing it. The manager then discards any edit the tool left uncommitted.
    /// # Errors
    /// Preserve document causes; temporary navigation is not entered on failure.
    fn suspend(&mut self, _context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        Ok(())
    }
    /// Release transient tool state without I/O. The manager independently cancels capture/edits.
    fn cancel(&mut self, reason: ToolCancelReason);
    /// Resolve feedback from the current shared state without allocating a native cursor.
    fn cursor(&self, view: &ToolView<'_>) -> CursorIntent;
    /// The tool's current settings, when it has any.
    fn options(&self) -> Option<ToolOptions> {
        None
    }
    /// Replace the tool's settings.
    /// # Errors
    /// Rejects settings this tool does not take; the default takes none.
    fn configure(&mut self, _options: ToolOptions) -> Result<(), CanvasError> {
        Err(CanvasError::Invalid("Tool has no settings"))
    }
    /// Whether Space belongs to this tool's own text/interaction protocol.
    fn captures_space(&self) -> bool {
        false
    }
    /// Presentation-only feedback for the current gesture, in document coordinates.
    fn overlay(&self) -> Option<ToolOverlay<'_>> {
        None
    }
    /// Act on a typed command; `Ok(false)` when this tool does not take it. A tool asks for the
    /// previously selected tool back through [`ToolContext::settle`].
    /// # Errors
    /// Preserve document causes; the tool's session state stays consistent on failure.
    fn command(
        &mut self,
        _command: ToolCommand,
        _context: &mut ToolContext<'_>,
    ) -> Result<bool, CanvasError> {
        Ok(false)
    }
    /// The tool's unresolved edit session. While one is open the tool cannot be changed, its
    /// preview survives temporary navigation and focus, geometry and policy changes, and
    /// lifecycle cancellation settles a gesture in flight into a checkpoint instead of
    /// discarding the session; an explicit cancel, a document change or shutdown cancels it.
    fn session(&self) -> Option<SessionState> {
        None
    }
}
