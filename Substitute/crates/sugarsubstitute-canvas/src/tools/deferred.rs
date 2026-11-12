// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coverage work a tool hands to the execution owner instead of running on the input thread.
//!
//! Rasterizing a large selection, or expanding, contracting and feathering a whole mask, can take
//! long enough to stall input. A tool describes such work as a [`DeferredEdit`]; the host runs it
//! as an owned job against the snapshot it was requested for, or the newer revision it was rebased
//! onto, and hands the result back. The result applies only while the document still shows the
//! revision the job read and no gesture is staging an edit there. Otherwise the work comes back
//! as [`DeferredCompletion::Rerun`] to be rebased and run again, so it lands after the undo, redo
//! or stroke that changed the document first. Work whose result is still unusable after
//! [`DeferredEdit::MAX_RUNS`] runs is abandoned, so a document that keeps changing cannot hold
//! the work queue forever.
//!
//! The job computes under its own working reservation, and its result moves to the document's
//! lease before the job ends: a document keeps results in its content and history, and must
//! not keep each job's whole working reservation admitted for as long as it does.
use super::ToolManager;
use crate::{CanvasError, Document, DocumentEdit, DocumentSnapshot, EditSessionId};
use std::time::Duration;
use sugarsubstitute_contracts::execution::{JobContext, MemoryLease};

/// What a finished deferred edit does to the document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Disposition {
    /// Publish as one undo step.
    Commit,
    /// Stage in the tool's edit session for a live preview, replacing any earlier preview; the
    /// tool commits or cancels it later.
    Preview,
}

/// Work computing one edit from the revision it reads. It may run more than once, each time
/// against a newer revision, when the document changes while it runs.
pub(super) type Work =
    Box<dyn Fn(&DocumentSnapshot, &JobContext) -> Result<DocumentEdit, CanvasError> + Send>;

/// Pending work and the exact revision it was requested against.
pub struct DeferredEdit {
    base: DocumentSnapshot,
    /// The lease of the document the result lands in, which takes over the result's charge.
    memory: MemoryLease,
    disposition: Disposition,
    request: u64,
    /// How many times this work has run already.
    runs: u8,
    work: Work,
}

/// A computed edit waiting to be applied on the input thread, carrying its work in case the
/// document moved on while it ran.
pub struct PreparedEdit {
    base: (u64, u64),
    disposition: Disposition,
    request: u64,
    runs: u8,
    edit: DocumentEdit,
    work: Work,
}

/// What finishing a deferred edit did.
#[derive(Debug)]
pub enum DeferredCompletion {
    /// The edit was published as one undo step or staged as the live preview.
    Applied,
    /// The edit changed nothing: its result matched the document, or a newer request of the
    /// open coverage modification superseded it.
    Unchanged,
    /// The document moved past the revision the work read, or a gesture is staging an edit
    /// there. Run the returned work again, ahead of work requested after it; like any waiting
    /// work, it is rebased onto the document's current revision just before it starts.
    Rerun(DeferredEdit),
    /// The result was still unusable after [`DeferredEdit::MAX_RUNS`] runs; the edit was
    /// abandoned and the document is unchanged.
    Abandoned,
}

impl DeferredCompletion {
    /// Whether the document or the live preview changed.
    pub const fn applied(&self) -> bool {
        matches!(self, Self::Applied)
    }
}

impl DeferredEdit {
    /// Most times one deferred edit runs. A result that is still unusable after this many runs
    /// abandons the edit instead of running it again.
    pub const MAX_RUNS: u8 = 8;

    pub(super) fn new(
        document: &Document,
        disposition: Disposition,
        request: u64,
        work: Work,
    ) -> Self {
        let (base, memory) = (document.snapshot(), document.memory().clone());
        Self { base, memory, disposition, request, runs: 0, work }
    }
    /// The revision this work reads.
    pub const fn base(&self) -> &DocumentSnapshot {
        &self.base
    }
    /// What the finished edit does to the document.
    pub const fn disposition(&self) -> Disposition {
        self.disposition
    }
    /// How many times this work has run already; a rerun counts its earlier runs.
    pub const fn runs(&self) -> u8 {
        self.runs
    }
    /// The manager-assigned order of this request, which [`ToolManager::deferred_failed`]
    /// takes when the work ends without a result.
    pub const fn request(&self) -> u64 {
        self.request
    }
    /// Read the document's current revision instead of the one this work was requested
    /// against. Work a tool requested while earlier work of the same document was still running
    /// reads that work's result this way, so successive gestures each land in order. The work
    /// itself is unchanged: it reshapes whatever content its base holds.
    /// # Errors
    /// Rejects a document of another identity.
    pub fn rebase(&mut self, document: &Document) -> Result<(), CanvasError> {
        let current = document.snapshot();
        if current.identity() != self.base.identity() {
            return Err(CanvasError::Invalid("Deferred work belongs to another document"));
        }
        self.base = current;
        self.memory = document.memory().clone();
        Ok(())
    }
    /// Run the work inside an owned execution job, then charge its result to the document's
    /// lease, so the job's working reservation is released when the job ends.
    /// # Errors
    /// Preserves the work's cancellation, deadline, memory and validation failures, and rejects
    /// a result the document's lease cannot hold.
    pub fn run(self, context: &JobContext) -> Result<PreparedEdit, CanvasError> {
        let edit = (self.work)(&self.base, context)?.adopt(&self.memory)?;
        Ok(PreparedEdit {
            base: (self.base.identity(), self.base.revision()),
            disposition: self.disposition,
            request: self.request,
            runs: self.runs.saturating_add(1),
            edit,
            work: self.work,
        })
    }
}

impl PreparedEdit {
    /// Whether the document still shows the revision this edit was computed from.
    pub fn current(&self, document: &Document) -> bool {
        let snapshot = document.snapshot();
        self.base == (snapshot.identity(), snapshot.revision())
    }
    pub(super) const fn disposition(&self) -> Disposition {
        self.disposition
    }
    /// The manager-assigned order of the request that produced this edit.
    pub(super) const fn request(&self) -> u64 {
        self.request
    }
    pub(super) fn into_edit(self) -> DocumentEdit {
        self.edit
    }
    /// Hand the work back to run again against `document`'s current revision under its
    /// original request order, or abandon it once it has run [`DeferredEdit::MAX_RUNS`] times.
    /// # Errors
    /// Rejects a document of another identity.
    pub(super) fn rerun(self, document: &Document) -> Result<DeferredCompletion, CanvasError> {
        let base = document.snapshot();
        if base.identity() != self.base.0 {
            return Err(CanvasError::Invalid("Deferred work belongs to another document"));
        }
        if self.runs >= DeferredEdit::MAX_RUNS {
            return Ok(DeferredCompletion::Abandoned);
        }
        Ok(DeferredCompletion::Rerun(DeferredEdit {
            base,
            memory: document.memory().clone(),
            disposition: self.disposition,
            request: self.request,
            runs: self.runs,
            work: self.work,
        }))
    }
}

impl std::fmt::Debug for DeferredEdit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeferredEdit")
            .field("base", &(self.base.identity(), self.base.revision()))
            .field("disposition", &self.disposition)
            .field("runs", &self.runs)
            .finish_non_exhaustive()
    }
}

impl ToolManager {
    /// Hand the latest requested coverage work to the host, which runs it as an owned job.
    pub fn take_deferred(&mut self) -> Option<DeferredEdit> {
        self.deferred.take()
    }
    /// Whether a tool's gesture is staging its own edit, as a brush stroke in progress does.
    /// Deferred work of this document should wait until it ends: a finished result would
    /// displace the staged edit, and the gesture's commit would outdate the result. A drag whose
    /// staged edit is its own deferred preview, as Move and Transform drags show, is not one.
    pub const fn staging_gesture(&self) -> bool {
        self.capture && self.edit.is_some() && self.previews.staged.is_none()
    }
    /// Apply finished coverage work. A result computed from a revision the document no longer
    /// shows, or finished while a gesture stages an edit, hands its work back to run again
    /// against the current revision. A preview replaces the tool's staged preview unless a newer
    /// one is staged or the previews were discarded after it was requested; a commit discards
    /// any staged preview and publishes one undo step. While a coverage modification is open
    /// only its latest request applies, and its commit restores the captured tool.
    /// # Errors
    /// Preserves validation, admission, identity and tool activation failures.
    pub fn complete_deferred(
        &mut self,
        prepared: PreparedEdit,
        document: &mut Document,
        now: Duration,
    ) -> Result<DeferredCompletion, CanvasError> {
        if self.modification.is_some() {
            return self.complete_modification(prepared, document, now);
        }
        let request = prepared.request();
        let obsolete = request <= self.previews.floor
            || self.previews.staged.is_some_and(|staged| staged > request);
        if prepared.disposition() == Disposition::Preview && obsolete {
            return Ok(DeferredCompletion::Unchanged);
        }
        if !prepared.current(document) || self.staging_gesture() {
            return prepared.rerun(document);
        }
        let applied = match prepared.disposition() {
            Disposition::Commit => {
                drop(self.edit.take());
                let floor = self.previews.floor.max(request);
                self.previews = super::context::PreviewOrder { floor, staged: None };
                document.apply(prepared.into_edit())?
            }
            Disposition::Preview => {
                self.stage_deferred(prepared, document)?;
                self.previews.staged = Some(request);
                true
            }
        };
        Ok(if applied { DeferredCompletion::Applied } else { DeferredCompletion::Unchanged })
    }
    /// Stage a finished preview in a fresh edit session, replacing the staged one.
    pub(super) fn stage_deferred(
        &mut self,
        prepared: PreparedEdit,
        document: &Document,
    ) -> Result<(), CanvasError> {
        let next =
            self.next_edit.checked_add(1).ok_or(CanvasError::Invalid("Edit identity exhausted"))?;
        let mut session = document.begin_edit(EditSessionId::new(next)?);
        session.stage(prepared.into_edit())?;
        self.next_edit = next;
        self.edit = Some(session);
        Ok(())
    }
}

/// Package work against the loaded document's current revision under the next request order.
pub(super) fn request(
    document: Option<&Document>,
    disposition: Disposition,
    next_request: &mut u64,
    work: Work,
) -> Result<DeferredEdit, CanvasError> {
    let document = document.ok_or(CanvasError::Invalid("No editable document"))?;
    let request = next_request
        .checked_add(1)
        .ok_or(CanvasError::Invalid("Deferred request order exhausted"))?;
    *next_request = request;
    Ok(DeferredEdit::new(document, disposition, request, work))
}
