// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route typed tool lifecycles with one shared navigation and document-edit boundary.
mod box_snap;
mod brush;
mod context;
mod contract;
mod coverage_commands;
mod coverage_shape;
mod coverage_work;
mod deferred;
mod lifecycle;
mod move_tool;
mod pan_zoom;
mod routing;
mod session;
mod shared_edges;
mod transform;
use crate::{
    CanvasError, CursorIntent, Document, DocumentEdit, DocumentSnapshot, EditPreview, EditSession,
    EditSessionId, LayerId, Navigation, NavigationCommand, NavigationCommandOutcome,
    NavigationInput, ScrollPhase, SnapGuide, overlay::ToolOverlay,
};
pub use brush::{BrushMode, BrushOptions, BrushTool};
pub use context::ToolContext;
pub use contract::{
    CanvasTool, PointerKind, PointerSample, Pressure, SessionAction, SessionKind, SessionState,
    ToolButton, ToolCancelReason, ToolCommand, ToolId, ToolInput, ToolModifiers, ToolOptions,
    ToolView, TransformAction,
};
pub use coverage_commands::{CoverageModification, CoverageOperation, ModifyPhase, ModifyTarget};
pub use coverage_shape::{CoverageShapeTool, ShapeKind, ShapeTarget};
pub use deferred::{DeferredCompletion, DeferredEdit, Disposition, PreparedEdit};
pub use move_tool::MoveTool;
pub use session::SESSION_CHECKPOINTS;
pub use shared_edges::SharedEdgesTool;
use std::time::Duration;
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
use sugarsubstitute_image_ops::ImageError;
pub use transform::TransformTool;

struct Registered {
    id: ToolId,
    tool: Box<dyn CanvasTool>,
    _memory: MemoryClaim,
}
/// Bounded tool registry and effective interaction state, independent of native controls.
/// Built-in PanZoom uses inline storage. Registered state is admitted by the caller's execution
/// lease; the manager creates neither scheduling nor document/history ownership.
pub struct ToolManager {
    navigation: Navigation,
    selected: ToolId,
    effective: ToolId,
    space: bool,
    capture: bool,
    pan_zoom: pan_zoom::PanZoom,
    registered: [Option<Registered>; 16],
    edit: Option<EditSession>,
    next_edit: u64,
    target: Option<LayerId>,
    deferred: Option<DeferredEdit>,
    next_request: u64,
    previews: context::PreviewOrder,
    modification: Option<coverage_commands::Modification>,
    previous: ToolId,
    settled: bool,
}
impl ToolManager {
    /// Create navigation as the selected/effective built-in tool with no heap allocation.
    pub fn new(navigation: Navigation) -> Self {
        Self {
            navigation,
            selected: ToolId::PAN_ZOOM,
            effective: ToolId::PAN_ZOOM,
            space: false,
            capture: false,
            pan_zoom: Default::default(),
            registered: std::array::from_fn(|_| None),
            edit: None,
            next_edit: 0,
            target: None,
            deferred: None,
            next_request: 0,
            previews: context::PreviewOrder::default(),
            modification: None,
            previous: ToolId::PAN_ZOOM,
            settled: false,
        }
    }
    /// Choose the layer editing tools act on, such as the active mask; `None` leaves them
    /// nothing to edit.
    pub fn set_target(&mut self, layer: Option<LayerId>) {
        self.target = layer;
    }
    /// The layer editing tools act on.
    pub const fn target(&self) -> Option<LayerId> {
        self.target
    }
    /// The staged gesture as a renderable snapshot, when a tool has one.
    pub fn preview_snapshot(&self) -> Option<DocumentSnapshot> {
        self.edit.as_ref().map(EditSession::snapshot)
    }
    /// Register bounded tool state with its original execution-backed memory charge.
    /// # Errors
    /// Rejects reserved/duplicate identities, capacity and admission before registration.
    pub fn register<T: CanvasTool + 'static>(
        &mut self,
        id: ToolId,
        tool: T,
        memory: &MemoryLease,
    ) -> Result<(), CanvasError> {
        if self.contains(id) {
            return Err(CanvasError::Invalid("Duplicate tool identity"));
        }
        let slot = self
            .registered
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or(CanvasError::Invalid("Tool registry is full"))?;
        let claim = memory.claim(size_of::<T>() as u64 + 128).map_err(ImageError::from)?;
        *slot = Some(Registered { id, tool: Box::new(tool), _memory: claim });
        Ok(())
    }
    /// The settings a registered tool uses now, including changes its own input made, such as
    /// a brush size the wheel changed. Built-in navigation has none.
    pub fn options(&self, id: ToolId) -> Option<ToolOptions> {
        self.registered.iter().flatten().find(|entry| entry.id == id)?.tool.options()
    }
    /// Replace a registered tool's settings. They apply from the tool's next observation, so a
    /// stroke in progress continues with the new size, hardness or opacity.
    /// # Errors
    /// Rejects unknown identities, built-in navigation and settings the tool does not take.
    pub fn configure(&mut self, id: ToolId, options: ToolOptions) -> Result<(), CanvasError> {
        self.registered
            .iter_mut()
            .flatten()
            .find(|entry| entry.id == id)
            .ok_or(CanvasError::Invalid("Unknown tool identity"))?
            .tool
            .configure(options)
    }
    /// Whether an identity is currently available for selection.
    pub fn contains(&self, id: ToolId) -> bool {
        id == ToolId::PAN_ZOOM || self.registered.iter().flatten().any(|entry| entry.id == id)
    }
    /// Persistently selected tool, including while temporary navigation is effective.
    pub const fn selected(&self) -> ToolId {
        self.selected
    }
    /// Tool currently receiving physical input.
    pub const fn effective(&self) -> ToolId {
        self.effective
    }
    /// Whether the effective tool owns pointer capture, including navigation and edit gestures.
    pub const fn captured(&self) -> bool {
        self.capture || self.navigation.captured()
    }
    /// Authoritative shared navigation state.
    pub const fn navigation(&self) -> &Navigation {
        &self.navigation
    }
    /// Whether a primary drag exports the content: navigation is the effective tool with no
    /// uncommitted edit taking priority, and the content cannot pan.
    pub fn allows_drag_out(&self) -> bool {
        self.effective == ToolId::PAN_ZOOM
            && self.edit.is_none()
            && self.navigation.allows_drag_out()
    }
    /// Command the shared camera without changing selected tools, staged edits or capture.
    /// A tool's live gesture retains ownership until its existing input lifecycle completes.
    /// # Errors
    /// Preserves navigation parameter, geometry and monotonic-time errors.
    pub fn navigate(
        &mut self,
        command: NavigationCommand,
        now: Duration,
    ) -> Result<NavigationCommandOutcome, CanvasError> {
        if self.captured() || (self.edit.is_some() && !self.holds_edit()) {
            return Ok(NavigationCommandOutcome::Denied);
        }
        self.navigation.command(command, now)
    }
    /// Stop a matching semantic camera destination without cancelling a tool or physical gesture.
    /// # Errors
    /// Preserves the navigation owner's monotonic-time and geometry failures.
    pub fn cancel_navigation(
        &mut self,
        target: crate::Viewport,
        mode: crate::ZoomMode,
        now: Duration,
    ) -> Result<bool, CanvasError> {
        if self.captured() || (self.edit.is_some() && !self.holds_edit()) {
            return Ok(false);
        }
        self.navigation.cancel_command(target, mode, now)
    }
    /// Advance motion from the application's injected clock without dispatching tool input.
    /// # Errors
    /// Rejects backward time and preserves camera errors.
    pub fn advance(&mut self, now: Duration) -> Result<(), CanvasError> {
        self.navigation.advance(now)
    }
    /// Supply current-monitor hertz; `None` uses 60 Hz. Fractional rates retain their precision.
    /// # Errors
    /// Rejects nonfinite or unsupported rates without changing motion.
    pub fn set_refresh_rate(&mut self, hertz: Option<f64>) -> Result<(), CanvasError> {
        self.navigation.set_refresh_rate(hertz)
    }
    /// Observe a gesture's transient metadata without granting document mutation authority.
    pub fn preview(&self) -> Option<EditPreview<'_>> {
        self.edit.as_ref().map(EditSession::preview)
    }
    /// Select a tool. While Space is held, update selection without displacing navigation.
    /// The tool selected before stays remembered, so a tool that settles returns to it.
    /// # Errors
    /// Preserves current selection/gesture on unknown identity, activation rejection or an
    /// unresolved edit session, which must be applied or cancelled first.
    pub fn select(
        &mut self,
        id: ToolId,
        document: Option<&Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        if !self.contains(id) {
            return Err(CanvasError::Invalid("Unknown tool identity"));
        }
        if id == self.selected {
            return Ok(());
        }
        self.refuse_during_session()?;
        if !self.space && self.modification.is_none() {
            self.switch(id, document, ToolCancelReason::SelectionChanged, now)?;
        }
        self.previous = self.selected;
        self.selected = id;
        Ok(())
    }
    /// Route one observation to the effective tool and the document's sole edit boundary.
    /// # Errors
    /// Preserves tool, navigation and document errors without swallowing cancellation/failure.
    pub fn input(
        &mut self,
        input: ToolInput,
        document: Option<&mut Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        let effective = self.effective;
        self.dispatch_to(effective, document, now, |tool, context| tool.input(input, context))
    }
    /// Resolve cursor intent without creating native cursor resources or touching image workers.
    pub fn cursor(&self, document: Option<&Document>) -> CursorIntent {
        self.active().map_or(CursorIntent::Arrow, |tool| {
            tool.cursor(&ToolView {
                navigation: &self.navigation,
                document: document.map(Document::snapshot),
                target: self.target,
            })
        })
    }
    /// The effective tool's presentation-only feedback, in document coordinates.
    pub fn overlay(&self) -> Option<ToolOverlay<'_>> {
        self.active().and_then(CanvasTool::overlay)
    }
    /// Smart guides of the snaps the effective tool's gesture holds, in document coordinates.
    pub fn snap_guides(&self) -> &[SnapGuide] {
        self.active().map_or(&[], CanvasTool::snap_guides)
    }
    fn active(&self) -> Option<&dyn CanvasTool> {
        if self.effective == ToolId::PAN_ZOOM {
            return Some(&self.pan_zoom);
        }
        self.registered
            .iter()
            .flatten()
            .find(|entry| entry.id == self.effective)
            .map(|entry| entry.tool.as_ref())
    }
}
#[cfg(test)]
mod tests;
