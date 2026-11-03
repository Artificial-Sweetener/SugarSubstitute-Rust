// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Edit sessions: typed commands, bounded session checkpoints and unified undo.
//!
//! Sessions follow the Python app's CuteCanvas policy: up to 256 checkpoints of their own, Undo
//! that stays inside the session and never passes its base, and tool changes refused until the
//! session is applied or cancelled. A session tool keeps only small parameters per checkpoint,
//! such as an offset or a transform; each change previews the whole result from the published
//! document in an owned job, so history memory never holds rasters.
use super::{
    CanvasTool, ContextParts, Disposition, SessionAction, SessionKind, SessionState,
    ToolCancelReason, ToolCommand, ToolContext, ToolId, ToolManager,
};
use crate::{CanvasError, Document, DocumentEdit, DocumentSnapshot};
use std::{collections::VecDeque, time::Duration};
use sugarsubstitute_contracts::execution::JobContext;

/// Most checkpoints one edit session keeps; the oldest is dropped first.
pub const SESSION_CHECKPOINTS: usize = 256;
/// Largest nudge the editing UI sends along each axis, in document pixels.
const MAX_NUDGE: i32 = 100;

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
    /// as applying or cancelling a transform does, restores the tool selected before it, or Pan
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
}

/// Bounded checkpoints of one session's parameters above its immutable base.
#[derive(Debug)]
pub(super) struct Checkpoints<T> {
    base: T,
    states: VecDeque<T>,
    applied: usize,
}
impl<T: Clone + PartialEq> Checkpoints<T> {
    pub(super) const fn new(base: T) -> Self {
        Self { base, states: VecDeque::new(), applied: 0 }
    }
    /// The parameters the session shows now.
    pub(super) fn current(&self) -> &T {
        self.applied.checked_sub(1).and_then(|index| self.states.get(index)).unwrap_or(&self.base)
    }
    /// The parameters the session started from.
    pub(super) const fn base(&self) -> &T {
        &self.base
    }
    /// Record a new checkpoint, discarding redoable ones; the oldest drops beyond the limit.
    /// Returns whether `state` differs from the current one.
    /// # Errors
    /// Rejects allocation failure without changing the checkpoints.
    pub(super) fn record(&mut self, state: T) -> Result<bool, CanvasError> {
        if &state == self.current() {
            return Ok(false);
        }
        if self.states.len() > self.applied {
            self.states.truncate(self.applied);
        }
        if self.states.len() == SESSION_CHECKPOINTS {
            self.states.pop_front();
        } else {
            self.states.try_reserve(1).map_err(sugarsubstitute_image_ops::ImageError::from)?;
        }
        self.states.push_back(state);
        self.applied = self.states.len();
        Ok(true)
    }
    pub(super) fn undo(&mut self) -> bool {
        let moved = self.applied > 0;
        self.applied = self.applied.saturating_sub(1);
        moved
    }
    pub(super) fn redo(&mut self) -> bool {
        let moved = self.applied < self.states.len();
        self.applied = (self.applied + 1).min(self.states.len());
        moved
    }
    /// Whether any checkpoint was ever recorded: the session is open even when undone to base.
    pub(super) fn changed(&self) -> bool {
        !self.states.is_empty()
    }
    pub(super) fn state(&self, kind: SessionKind) -> SessionState {
        SessionState { kind, undo: self.applied > 0, redo: self.applied < self.states.len() }
    }
}

/// Work computing one whole edit from the published document; it may run again against a
/// newer revision when the document changes while it runs.
pub(super) type EditWork =
    Box<dyn Fn(&DocumentSnapshot, &JobContext) -> Result<DocumentEdit, CanvasError> + Send>;

/// One parametric edit previewed through deferred work and published from its latest preview.
#[derive(Debug, Default)]
pub(super) struct Previewed {
    request: Option<u64>,
}
impl Previewed {
    /// Request a preview of `work`, replacing any earlier one.
    pub(super) fn preview(
        &mut self,
        context: &mut ToolContext<'_>,
        work: EditWork,
    ) -> Result<(), CanvasError> {
        context.defer(Disposition::Preview, work)?;
        self.request = Some(context.latest_request());
        Ok(())
    }
    /// Publish `work` as one undo step: the staged preview when it is the latest and current,
    /// otherwise through an owned job.
    pub(super) fn publish(
        &mut self,
        context: &mut ToolContext<'_>,
        work: EditWork,
    ) -> Result<(), CanvasError> {
        let latest = self.request.take();
        if latest.is_some() && context.staged_request() == latest {
            return context.commit_edit().map(drop);
        }
        context.cancel_edit();
        context.defer(Disposition::Commit, work)
    }
    /// Drop the preview.
    pub(super) fn discard(&mut self, context: &mut ToolContext<'_>) {
        self.request = None;
        context.cancel_edit();
    }
}
