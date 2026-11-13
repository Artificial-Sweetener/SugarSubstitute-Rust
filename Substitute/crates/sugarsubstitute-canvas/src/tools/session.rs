// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Edit-session primitives for session tools: bounded checkpoints and previewed publication.
//!
//! Sessions follow the Python app's CuteCanvas policy: up to 256 checkpoints of their own, Undo
//! that stays inside the session and never passes its base, and tool changes refused until the
//! session is applied or cancelled. A session tool keeps only small parameters per checkpoint,
//! such as an offset or a transform; each change previews the whole result from the published
//! document in an owned job, so history memory never holds rasters.
use super::{Disposition, SessionKind, SessionState, ToolContext};
use crate::{CanvasError, DocumentEdit, DocumentSnapshot};
use std::collections::VecDeque;
use sugarsubstitute_contracts::execution::JobContext;

/// Most checkpoints one edit session keeps; the oldest is dropped first.
pub const SESSION_CHECKPOINTS: usize = 256;

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
