// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run deferred coverage work and selection tracing as owned blocking jobs, one of each at a
//! time, beside the frame pipeline's CPU lanes.
//!
//! Deferred edits wait in request order, so successive gestures each land: one requested while
//! earlier work of its document was still running is rebased onto that document's current
//! revision just before it starts, and so reads the earlier result. Work waits while a gesture
//! of its document stages an edit, such as a brush stroke in progress. A newer preview replaces
//! a waiting preview of the same document, because only the latest preview is ever staged. A
//! finished edit applies to the document it read, shown or parked, while that document still
//! shows the revision it read. When an undo, redo or stroke changed the document while the job
//! ran, the work returns to the head of the queue and runs again against the newer revision, so
//! it lands after that change; the tool manager abandons work whose result is still stale after
//! [`DeferredEdit::MAX_RUNS`] runs.
use super::*;
use std::collections::VecDeque;
use sugarsubstitute_canvas::{
    BoundaryWork, CoverageBoundary, DeferredCompletion, DeferredEdit, Disposition, Document,
    PreparedEdit, ToolManager,
};
use sugarsubstitute_execution::OwnedInputError;
use sugarsubstitute_image_ops::Coverage;

/// Working storage a coverage job may need per document pixel: a result coverage and separable
/// filter scratch at two bytes per value; the source coverage is shared, not copied.
const WORK_BYTES_PER_PIXEL: u64 = 6;
/// Working storage a selection trace may need per document pixel, for its boundary runs.
const TRACE_BYTES_PER_PIXEL: u64 = 1;
/// Fixed working storage of one coverage job.
const WORK_BASE_BYTES: u64 = 1 << 20;
/// Most deferred edits that may wait behind the running one.
const WAITING_EDITS: usize = 32;

/// The running and waiting coverage work and the running selection trace.
#[derive(Default)]
pub(super) struct EditingWork {
    /// The running edit: its document identity, its request order and its job.
    running: Option<(u64, u64, OwnedJob<PreparedEdit, WorkError>)>,
    /// Edits waiting to start, oldest first.
    waiting: VecDeque<DeferredEdit>,
    tracing: Option<([u8; 32], OwnedJob<CoverageBoundary, WorkError>)>,
    trace: Option<BoundaryWork>,
    /// Working storage the waiting trace reserves.
    trace_bytes: u64,
}

impl EditingWork {
    /// Wait for a job slot behind every earlier edit. A preview replaces waiting previews of its
    /// document. Returns `false`, keeping the queue unchanged, when it is full.
    pub(super) fn queue(&mut self, work: DeferredEdit) -> bool {
        if work.disposition() == Disposition::Preview {
            let identity = work.base().identity();
            self.waiting.retain(|waiting| {
                waiting.disposition() != Disposition::Preview
                    || waiting.base().identity() != identity
            });
        }
        if self.waiting.len() >= WAITING_EDITS {
            return false;
        }
        self.waiting.push_back(work);
        true
    }

    /// Put work whose result went stale back at the head of the queue, ahead of everything
    /// requested after it. It held the running slot, so the waiting bound does not refuse it.
    /// Returns `false`, dropping it, for a preview that a newer waiting preview of its document
    /// supersedes.
    fn requeue(&mut self, work: DeferredEdit) -> bool {
        let identity = work.base().identity();
        let superseded = work.disposition() == Disposition::Preview
            && self.waiting.iter().any(|waiting| {
                waiting.disposition() == Disposition::Preview
                    && waiting.base().identity() == identity
            });
        if !superseded {
            self.waiting.push_front(work);
        }
        !superseded
    }

    pub(super) fn in_flight(&self) -> bool {
        self.running.is_some() || self.tracing.is_some()
    }

    /// Stop running work for close and drop what has not started.
    pub(super) fn cancel(&mut self) {
        if let Some((_, _, job)) = &self.running {
            job.cancel();
        }
        if let Some((_, job)) = &self.tracing {
            job.cancel();
        }
        self.waiting.clear();
        self.trace = None;
    }
}

impl Session {
    /// Working storage for work over a document of `extent` needing `per_pixel` bytes.
    fn work_bytes(
        extent: sugarsubstitute_image_ops::Extent,
        per_pixel: u64,
    ) -> Result<u64, Box<dyn Error>> {
        extent
            .pixels()
            .checked_mul(per_pixel)
            .and_then(|bytes| bytes.checked_add(WORK_BASE_BYTES))
            .ok_or_else(|| "Coverage work reservation overflow".into())
    }

    /// The tools and document with snapshot identity `identity`, shown or parked.
    fn editing_document(&self, identity: u64) -> Option<(&ToolManager, &Document)> {
        let shown = self
            .document
            .scene
            .as_ref()
            .is_some_and(|scene| scene.document.snapshot().identity() == identity);
        let state =
            if shown { Some(&*self.document) } else { self.inputs.parked_by_identity(identity) }?;
        Some((&state.tools, &state.scene.as_ref()?.document))
    }

    /// Start the oldest waiting deferred edit and the waiting selection trace when their slots
    /// are free. Saturated admission keeps them waiting for the completion wake; a gesture
    /// staging an edit in the oldest edit's document keeps it waiting until the gesture's input
    /// ends it.
    pub(super) fn submit_editing_work(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Ok(());
        }
        while self.editing.work.running.is_none()
            && let Some(identity) =
                self.editing.work.waiting.front().map(|work| work.base().identity())
        {
            let staging = self.editing_document(identity).map(|(tools, _)| tools.staging_gesture());
            if staging == Some(true) {
                break;
            }
            let mut work =
                self.editing.work.waiting.pop_front().ok_or("Coverage work left the queue")?;
            // Earlier work, an undo or a stroke may have changed the document since this read it.
            match self.editing_document(identity).map(|(_, document)| work.rebase(document)) {
                Some(Ok(())) => {}
                Some(Err(cause)) => return Err(cause.into()),
                None => {
                    tracing::debug!(
                        operation = "canvas_coverage_work",
                        document_id = identity,
                        "Coverage work of a retired document was dropped"
                    );
                    continue;
                }
            }
            let bytes = Self::work_bytes(work.base().extent(), WORK_BYTES_PER_PIXEL)?;
            let spec = self.event_spec(
                Operation::Infrastructure,
                JobClass::InfrastructureOrdinary,
                bytes,
                None,
            )?;
            let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
            let request = work.request();
            let mut slot = Some(work);
            match runtime.client.submit_blocking_owned_input(
                &runtime.scope,
                spec,
                &mut slot,
                |work, context| work.run(&context).map_err(WorkError::from),
            ) {
                Ok(job) => self.editing.work.running = Some((identity, request, job)),
                Err(OwnedInputError::Admission(cause)) if activity::retryable(&cause) => {
                    if let Some(work) = slot {
                        self.editing.work.waiting.push_front(work);
                    }
                    break;
                }
                Err(cause) => return Err(cause.into()),
            }
        }
        if self.editing.work.tracing.is_none()
            && let Some(work) = &self.editing.work.trace
        {
            let fingerprint = work.fingerprint();
            let bytes = self.editing.work.trace_bytes;
            let spec = self.event_spec(
                Operation::Infrastructure,
                JobClass::InfrastructureOrdinary,
                bytes,
                None,
            )?;
            let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
            match runtime.client.submit_blocking_owned_input(
                &runtime.scope,
                spec,
                &mut self.editing.work.trace,
                |work, context| work.run(&context).map_err(WorkError::from),
            ) {
                Ok(job) => self.editing.work.tracing = Some((fingerprint, job)),
                Err(OwnedInputError::Admission(cause)) if activity::retryable(&cause) => {}
                Err(cause) => return Err(cause.into()),
            }
        }
        Ok(())
    }

    /// Ask for the outline of the shown selection when it is neither traced nor being traced.
    pub(super) fn request_outline(
        &mut self,
        selection: Option<&Coverage>,
    ) -> Result<(), Box<dyn Error>> {
        if self.editing.work.tracing.is_some() || self.editing.work.trace.is_some() {
            return Ok(());
        }
        if let Some(selection) = selection {
            self.editing.work.trace_bytes =
                Self::work_bytes(selection.extent(), TRACE_BYTES_PER_PIXEL)?;
        }
        self.editing.work.trace = self.editing.outlines.request(selection);
        self.submit_editing_work()
    }

    /// Observe finished coverage work and traces, applying what is still current, then start
    /// the next waiting edit.
    pub(in crate::native_canvas::session) fn consume_editing_work(
        &mut self,
    ) -> Result<(), Box<dyn Error>> {
        let mut finished = false;
        if let Some(completion) =
            self.editing.work.running.as_mut().and_then(|(_, _, job)| job.try_take_completion())
            && let Some((identity, request, job)) = self.editing.work.running.take()
        {
            finished = true;
            Self::observe_terminal(&job, &completion.outcome);
            match Self::terminal(completion.outcome) {
                Ok(prepared) if !self.closing => self.complete_editing(identity, prepared),
                Ok(_) => {}
                Err(cause) => {
                    tracing::warn!(operation = "canvas_coverage_work", document_id = identity, cause = %cause, "Coverage work did not finish; the document is unchanged");
                    self.editing_work_failed(identity, request)?;
                }
            }
        }
        if let Some(completion) =
            self.editing.work.tracing.as_mut().and_then(|(_, job)| job.try_take_completion())
            && let Some((fingerprint, job)) = self.editing.work.tracing.take()
        {
            Self::observe_terminal(&job, &completion.outcome);
            match Self::terminal(completion.outcome) {
                Ok(boundary) if !self.closing => self.editing.outlines.complete(boundary),
                Ok(_) => {}
                Err(cause) => {
                    self.editing.outlines.abandon(fingerprint);
                    tracing::warn!(operation = "canvas_selection_outline", cause = %cause, "Selection outline tracing did not finish");
                }
            }
        }
        // Finished, failed or abandoned work may have ended the shown document's modification.
        if self.document.tools.modifying().is_none() {
            self.editing.modification = None;
        }
        if finished { self.submit_editing_work() } else { Ok(()) }
    }

    /// Apply a finished edit to the document it read, wherever that document is held.
    fn complete_editing(&mut self, identity: u64, prepared: PreparedEdit) {
        let now = self.navigation_time();
        let Some(document) = self.editing_document_mut(identity) else {
            return;
        };
        let Some(scene) = document.scene.as_mut() else {
            return;
        };
        match document.tools.complete_deferred(prepared, &mut scene.document, now) {
            Ok(DeferredCompletion::Applied) => {}
            Ok(DeferredCompletion::Unchanged) => {
                tracing::debug!(
                    operation = "canvas_coverage_work",
                    document_id = identity,
                    "Finished coverage work changed nothing: it was superseded or empty"
                );
            }
            Ok(DeferredCompletion::Rerun(work)) => {
                let runs = work.runs();
                if self.editing.work.requeue(work) {
                    tracing::debug!(
                        operation = "canvas_coverage_work",
                        document_id = identity,
                        runs,
                        "The document changed while coverage work ran; it runs again"
                    );
                } else {
                    tracing::debug!(
                        operation = "canvas_coverage_work",
                        document_id = identity,
                        "A newer preview superseded stale preview work"
                    );
                }
            }
            Ok(DeferredCompletion::Abandoned) => {
                tracing::warn!(
                    operation = "canvas_coverage_work",
                    document_id = identity,
                    runs = DeferredEdit::MAX_RUNS,
                    "The document kept changing while coverage work ran; the edit was abandoned"
                );
            }
            Err(cause) => {
                tracing::warn!(operation = "canvas_coverage_work", document_id = identity, cause = %cause, "Finished coverage work could not be applied");
            }
        }
    }

    /// The document state with snapshot identity `identity`, shown or parked.
    fn editing_document_mut(&mut self, identity: u64) -> Option<&mut document::DocumentState> {
        let shown = self
            .document
            .scene
            .as_ref()
            .is_some_and(|scene| scene.document.snapshot().identity() == identity);
        if shown { Some(&mut *self.document) } else { self.inputs.parked_mut(identity) }
    }

    /// Coverage work for `request` of document `identity` ended without a result: an open
    /// modification waiting on it ends as Cancel would end it, so its document's undo and tools
    /// are not left blocked.
    fn editing_work_failed(&mut self, identity: u64, request: u64) -> Result<(), Box<dyn Error>> {
        let now = self.navigation_time();
        let Some(document) = self.editing_document_mut(identity) else {
            return Ok(());
        };
        let scene = document.scene.as_ref().map(|scene| &scene.document);
        if document.tools.deferred_failed(request, scene, now)? {
            tracing::warn!(
                operation = "canvas_coverage_modification",
                document_id = identity,
                "A coverage modification's work did not finish; the modification was cancelled"
            );
        }
        Ok(())
    }

    /// Stop editing work for close and release traced outlines, whose storage the scope's
    /// settlement waits for; running jobs stay observed until their terminal outcomes.
    pub(in crate::native_canvas::session) fn close_editing(&mut self) {
        let mut work = std::mem::take(&mut self.editing.work);
        work.cancel();
        self.editing = Editing { work, ..Editing::default() };
    }
}
