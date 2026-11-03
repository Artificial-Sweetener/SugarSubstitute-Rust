// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route typed tool lifecycles with one shared navigation and document-edit boundary.
mod context;
mod contract;
mod coverage_commands;
mod coverage_shape;
mod coverage_work;
mod deferred;
mod pan_zoom;
use crate::{
    CanvasError, CursorIntent, Document, DocumentEdit, DocumentSnapshot, EditPreview, EditSession,
    EditSessionId, LayerId, Navigation, NavigationCommand, NavigationCommandOutcome,
    NavigationInput, ScrollPhase, overlay::ToolOverlay,
};
pub use context::ToolContext;
pub use contract::{
    CanvasTool, PointerKind, PointerSample, Pressure, ToolButton, ToolCancelReason, ToolId,
    ToolInput, ToolModifiers, ToolView,
};
pub use coverage_commands::{CoverageModification, CoverageOperation, ModifyPhase, ModifyTarget};
pub use coverage_shape::{CoverageShapeTool, ShapeKind, ShapeTarget};
pub use deferred::{DeferredEdit, Disposition, PreparedEdit};
use std::time::Duration;
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
use sugarsubstitute_image_ops::ImageError;

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
    modification: Option<coverage_commands::Modification>,
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
            modification: None,
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
    /// Hand the latest requested coverage work to the host, which runs it as an owned job.
    pub fn take_deferred(&mut self) -> Option<DeferredEdit> {
        self.deferred.take()
    }
    /// Apply finished coverage work. Work computed from a revision the document no longer shows
    /// is stale and changes nothing. A preview replaces the tool's staged preview; a commit
    /// discards any staged preview and publishes one undo step. While a coverage modification
    /// is open only its latest request applies, and its commit restores the captured tool.
    /// # Errors
    /// Preserves validation, admission, identity and tool activation failures.
    pub fn complete_deferred(
        &mut self,
        prepared: PreparedEdit,
        document: &mut Document,
        now: Duration,
    ) -> Result<bool, CanvasError> {
        if !prepared.current(document) {
            return Ok(false);
        }
        if self.modification.is_some() {
            return self.complete_modification(prepared, document, now);
        }
        drop(self.edit.take());
        match prepared.disposition() {
            Disposition::Commit => document.apply(prepared.into_edit()),
            Disposition::Preview => {
                let next = self
                    .next_edit
                    .checked_add(1)
                    .ok_or(CanvasError::Invalid("Edit identity exhausted"))?;
                let mut session = document.begin_edit(EditSessionId::new(next)?);
                session.stage(prepared.into_edit())?;
                self.next_edit = next;
                self.edit = Some(session);
                Ok(true)
            }
        }
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
        if self.captured() || (self.edit.is_some() && self.modification.is_none()) {
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
        if self.captured() || (self.edit.is_some() && self.modification.is_none()) {
            return Ok(false);
        }
        self.navigation.cancel_command(target, mode, now)
    }
    /// Park this document at its qualified native-accepted camera and retire active gestures.
    /// The caller supplies evidence for the current document/source. Capture, temporary tools
    /// and uncommitted edits follow the existing document-change cancellation lifecycle.
    ///
    /// # Errors
    /// Rejects camera/time before cancellation and preserves tool reactivation failures.
    pub fn freeze_at(
        &mut self,
        accepted: crate::Viewport,
        document: Option<&Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        let mut navigation = self.navigation;
        navigation.freeze_at(accepted, now)?;
        self.cancel(ToolCancelReason::DocumentChanged, document, now)?;
        self.navigation = navigation;
        Ok(())
    }
    /// Advance motion from the application's injected clock without dispatching tool input.
    /// # Errors
    /// Rejects backward time and preserves camera errors.
    pub fn advance(&mut self, now: Duration) -> Result<(), CanvasError> {
        self.navigation.advance(now)
    }
    /// Change physical geometry and cancel gestures whose pointer basis has changed.
    /// # Errors
    /// Rejects geometry/time before cancellation; preserves selected-tool activation errors.
    pub fn resize(
        &mut self,
        extent: sugarsubstitute_image_ops::Extent,
        document: Option<&Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        let mut navigation = self.navigation;
        navigation.resize(extent, now)?;
        if extent == self.navigation.viewport().surface() {
            self.navigation = navigation;
            return Ok(());
        }
        self.cancel(ToolCancelReason::SurfaceChanged, document, now)?;
        self.navigation = navigation;
        Ok(())
    }
    /// Replace content bounds and cancel capture and uncommitted edits from the previous document.
    /// # Errors
    /// Rejects geometry/time before cancellation; preserves selected-tool activation errors.
    pub fn set_content(
        &mut self,
        extent: Option<sugarsubstitute_image_ops::Extent>,
        document: Option<&Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        let mut navigation = self.navigation;
        navigation.set_content(extent, now)?;
        self.cancel(ToolCancelReason::DocumentChanged, document, now)?;
        self.navigation = navigation;
        Ok(())
    }
    /// Revoke navigation gestures when locking or reserving drag input for the host.
    /// # Errors
    /// Preserves clock and selected-tool activation errors during cancellation.
    pub fn set_policy(
        &mut self,
        locked: bool,
        drag_out: bool,
        document: Option<&Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        self.navigation.advance(now)?;
        if locked || drag_out {
            self.cancel(ToolCancelReason::Locked, document, now)?;
        }
        self.navigation.set_policy(locked, drag_out);
        Ok(())
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
    /// # Errors
    /// Preserves current selection/gesture on unknown identity or activation rejection.
    pub fn select(
        &mut self,
        id: ToolId,
        document: Option<&Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        if !self.contains(id) {
            return Err(CanvasError::Invalid("Unknown tool identity"));
        }
        if !self.space && self.modification.is_none() {
            self.switch(id, document, ToolCancelReason::SelectionChanged, now)?;
        }
        self.selected = id;
        Ok(())
    }
    /// Enter/leave temporary Space navigation without replacing persistent selection.
    /// # Errors
    /// Preserves activation errors; current capture and uncommitted edits remain bounded.
    pub fn temporary_navigation(
        &mut self,
        held: bool,
        document: Option<&Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        if held == self.space {
            return Ok(());
        }
        if held && self.active().is_some_and(CanvasTool::captures_space) {
            return Ok(());
        }
        if self.modification.is_some() {
            // Coverage modification already holds navigation; the selection returns after it.
            self.space = held;
            return Ok(());
        }
        let id = if held { ToolId::PAN_ZOOM } else { self.selected };
        self.switch(id, document, ToolCancelReason::TemporaryNavigation, now)?;
        self.space = held;
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
        let mut context = ToolContext {
            navigation: &mut self.navigation,
            document,
            edit: &mut self.edit,
            next_edit: &mut self.next_edit,
            capture: &mut self.capture,
            target: self.target,
            deferred: &mut self.deferred,
            next_request: &mut self.next_request,
            now,
        };
        if self.effective == ToolId::PAN_ZOOM {
            self.pan_zoom.input(input, &mut context)
        } else {
            let entry = self
                .registered
                .iter_mut()
                .flatten()
                .find(|entry| entry.id == self.effective)
                .ok_or(CanvasError::Invalid("Effective tool is not registered"))?;
            entry.tool.input(input, &mut context)
        }
    }
    /// Resolve cursor intent without creating native cursor resources or touching image workers.
    pub fn cursor(&self, document: Option<&Document>) -> CursorIntent {
        self.active().map_or(CursorIntent::Arrow, |tool| {
            tool.cursor(&ToolView {
                navigation: &self.navigation,
                document: document.map(Document::snapshot),
            })
        })
    }
    /// The effective tool's presentation-only feedback, in document coordinates.
    pub fn overlay(&self) -> Option<ToolOverlay<'_>> {
        self.active().and_then(CanvasTool::overlay)
    }
    /// Cancel capture and staged edits, restoring persistent selection after lifecycle loss.
    /// An unresolved coverage modification survives focus, geometry and policy changes with its
    /// preview; the person cancelling, a document change or shutdown cancels it too.
    /// # Errors
    /// Preserves clock and activation failures; staged gesture edits are always discarded.
    pub fn cancel(
        &mut self,
        reason: ToolCancelReason,
        document: Option<&Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        if self.modification.is_some() {
            return self.interrupt_modification(reason, document, now);
        }
        self.cancel_active(reason);
        self.capture = false;
        drop(self.edit.take());
        self.deferred = None;
        self.navigation.input(NavigationInput::Cancel, now)?;
        if self.space {
            self.switch(self.selected, document, reason, now)?;
            self.space = false;
        }
        Ok(())
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
    fn cancel_active(&mut self, reason: ToolCancelReason) {
        if self.effective == ToolId::PAN_ZOOM {
            self.pan_zoom.cancel(reason);
        } else if let Some(entry) =
            self.registered.iter_mut().flatten().find(|entry| entry.id == self.effective)
        {
            entry.tool.cancel(reason);
        }
    }
    fn switch(
        &mut self,
        id: ToolId,
        document: Option<&Document>,
        reason: ToolCancelReason,
        now: Duration,
    ) -> Result<(), CanvasError> {
        if id == self.effective {
            return Ok(());
        }
        self.navigation.advance(now)?;
        let mut navigation = self.navigation;
        navigation.input(NavigationInput::Cancel, now)?;
        let view = ToolView { navigation: &navigation, document: document.map(Document::snapshot) };
        if id == ToolId::PAN_ZOOM {
            self.pan_zoom.activate(&view)?;
        } else {
            self.registered
                .iter_mut()
                .flatten()
                .find(|entry| entry.id == id)
                .ok_or(CanvasError::Invalid("Unknown tool identity"))?
                .tool
                .activate(&view)?;
        }
        self.cancel_active(reason);
        self.capture = false;
        self.navigation = navigation;
        drop(self.edit.take());
        self.effective = id;
        Ok(())
    }
}
#[cfg(test)]
mod tests;
