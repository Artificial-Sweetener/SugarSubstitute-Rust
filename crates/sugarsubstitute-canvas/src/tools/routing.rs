// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route typed commands, session state and unified undo to the selected tool, and run every tool
//! action with the shared edit boundary.
//!
//! Sessions follow the Python app's CuteCanvas policy: Undo stays inside an open session and
//! never passes its base, and tool changes are refused until the session is applied or
//! cancelled. A tool that settles, as the Transform toolbar's Apply and Cancel do, gets the tool
//! selected before it back.
use super::{
    CanvasTool, SessionAction, SessionKind, SessionState, ToolCancelReason, ToolCommand,
    ToolContext, ToolId, ToolManager,
};
use crate::{CanvasError, Document};
use std::time::Duration;

/// Largest nudge the editing UI sends along each axis, in document pixels.
const MAX_NUDGE: i32 = 100;

/// The tool an action runs on and the context it runs with, borrowed from disjoint state.
struct ContextParts<'a> {
    tool: &'a mut dyn CanvasTool,
    context: ToolContext<'a>,
}

impl ToolManager {
    /// The unresolved edit session the editing UI reports: an open coverage modification, or the
    /// selected tool's session.
    pub fn session(&self) -> Option<SessionState> {
        if self.modification.is_some() {
            return Some(SessionState { kind: SessionKind::Modify, undo: false, redo: false });
        }
        self.selected_tool()?.session()
    }
    /// Deliver a typed command to the selected tool, even while Space navigation is effective.
    /// Escape a tool does not take cancels its gesture through the lifecycle, as before; during
    /// a coverage modification Escape cancels the modification. A tool settling its session,
    /// as the Transform toolbar's Apply and Cancel do, restores the tool selected before it, or Pan
    /// & Zoom. Returns whether anything acted.
    /// # Errors
    /// Rejects nudges beyond ±100 pixels and preserves tool, activation and document failures.
    pub fn command(
        &mut self,
        command: ToolCommand,
        document: &mut Document,
        now: Duration,
    ) -> Result<bool, CanvasError> {
        if let ToolCommand::Nudge(delta) = command
            && delta.iter().any(|value| !(-MAX_NUDGE..=MAX_NUDGE).contains(value))
        {
            return Err(CanvasError::Invalid("Nudges are at most 100 pixels"));
        }
        if self.modification.is_some() {
            if matches!(command, ToolCommand::Cancel | ToolCommand::Session(SessionAction::Cancel))
            {
                self.interrupt_modification(ToolCancelReason::Requested, Some(document), now)?;
                return Ok(true);
            }
            return Ok(false);
        }
        let selected = self.selected;
        if self.dispatch_to(selected, Some(document), now, |tool, context| {
            tool.command(command, context)
        })? {
            return Ok(true);
        }
        if command != ToolCommand::Cancel {
            return Ok(false);
        }
        let active = self.captured() || self.edit.is_some();
        self.cancel(ToolCancelReason::Requested, Some(document), now)?;
        Ok(active)
    }
    /// Undo for the editing UI: inside an open session it steps back the session, never past
    /// its base; otherwise it cancels any uncommitted gesture and undoes the document.
    /// # Errors
    /// Preserves tool and document failures.
    pub fn undo(&mut self, document: &mut Document, now: Duration) -> Result<bool, CanvasError> {
        self.history(SessionAction::Undo, document, now)
    }
    /// Redo for the editing UI, with the same session boundary as [`Self::undo`].
    /// # Errors
    /// Preserves tool and document failures.
    pub fn redo(&mut self, document: &mut Document, now: Duration) -> Result<bool, CanvasError> {
        self.history(SessionAction::Redo, document, now)
    }
    fn history(
        &mut self,
        action: SessionAction,
        document: &mut Document,
        now: Duration,
    ) -> Result<bool, CanvasError> {
        if self.modification.is_some() {
            return Ok(false);
        }
        if self.session_open() {
            return self.command(ToolCommand::Session(action), document, now);
        }
        if self.captured() || self.edit.is_some() {
            self.cancel(ToolCancelReason::Requested, Some(document), now)?;
        }
        match action {
            SessionAction::Undo => document.undo(),
            _ => document.redo(),
        }
    }
    /// Whether the selected tool holds an unresolved session.
    pub(super) fn session_open(&self) -> bool {
        self.selected_tool().is_some_and(|tool| tool.session().is_some())
    }
    /// Whether staged preview content belongs to an unresolved modification or session rather
    /// than to a gesture.
    pub(super) fn holds_edit(&self) -> bool {
        self.modification.is_some() || self.session_open()
    }
    pub(super) fn refuse_during_session(&self) -> Result<(), CanvasError> {
        if self.session_open() {
            return Err(CanvasError::Invalid("An edit session is unresolved"));
        }
        Ok(())
    }
    /// Drop staged content and make every outstanding preview obsolete.
    pub(super) fn discard_edit(&mut self) {
        drop(self.edit.take());
        self.previews.floor = self.next_request;
        self.previews.staged = None;
    }
    fn selected_tool(&self) -> Option<&dyn CanvasTool> {
        self.registered
            .iter()
            .flatten()
            .find(|entry| entry.id == self.selected)
            .map(|entry| entry.tool.as_ref())
    }
    /// Run one action of the tool `id` with the shared edit boundary, then honour a request to
    /// settle back to the previously selected tool.
    pub(super) fn dispatch_to<T>(
        &mut self,
        id: ToolId,
        mut document: Option<&mut Document>,
        now: Duration,
        action: impl FnOnce(&mut dyn CanvasTool, &mut ToolContext<'_>) -> Result<T, CanvasError>,
    ) -> Result<T, CanvasError> {
        let ContextParts { tool, mut context } =
            self.context_for(id, document.as_deref_mut(), now)?;
        let result = action(tool, &mut context);
        if std::mem::take(&mut self.settled) {
            self.restore_previous(document.as_deref(), now)?;
        }
        result
    }
    /// Select the tool chosen before the current one, falling back to Pan & Zoom.
    fn restore_previous(
        &mut self,
        document: Option<&Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        let previous = self.previous;
        if previous != self.selected
            && self.contains(previous)
            && self.select(previous, document, now).is_ok()
        {
            return Ok(());
        }
        self.select(ToolId::PAN_ZOOM, document, now)
    }
    /// Borrow the tool `id` and a context over the rest of the interaction state.
    fn context_for<'a>(
        &'a mut self,
        id: ToolId,
        document: Option<&'a mut Document>,
        now: Duration,
    ) -> Result<ContextParts<'a>, CanvasError> {
        let tool: &mut dyn CanvasTool = if id == ToolId::PAN_ZOOM {
            &mut self.pan_zoom
        } else {
            self.registered
                .iter_mut()
                .flatten()
                .find(|entry| entry.id == id)
                .ok_or(CanvasError::Invalid("Tool is not registered"))?
                .tool
                .as_mut()
        };
        let context = ToolContext {
            navigation: &mut self.navigation,
            document,
            edit: &mut self.edit,
            next_edit: &mut self.next_edit,
            capture: &mut self.capture,
            target: self.target,
            deferred: &mut self.deferred,
            next_request: &mut self.next_request,
            previews: &mut self.previews,
            settled: &mut self.settled,
            now,
        };
        Ok(ContextParts { tool, context })
    }
}
