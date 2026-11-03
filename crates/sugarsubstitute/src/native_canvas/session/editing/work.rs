// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run deferred coverage work and selection tracing as owned blocking jobs, one of each at a
//! time, beside the frame pipeline's CPU lanes.
//!
//! Deferred edits wait in request order, so successive gestures each land: one requested while
//! earlier work of its document was still running is rebased onto that document's current
//! revision just before it starts, and so reads the earlier result. A newer preview replaces a
//! waiting preview of the same document, because only the latest preview is ever staged. A
//! finished edit applies to the document it read, shown or parked, and only while that document
//! still shows the revision it read; anything else is stale and changes nothing.
use super::*;
use std::collections::VecDeque;
use sugarsubstitute_canvas::{
    BoundaryWork, CoverageBoundary, DeferredEdit, Disposition, Document, PreparedEdit,
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
/// Longest a coverage job may run.
const WORK_DEADLINE: Duration = Duration::from_secs(120);
/// Most deferred edits that may wait behind the running one.
const WAITING_EDITS: usize = 32;

/// The running and waiting coverage work and the running selection trace.
#[derive(Default)]
pub(super) struct EditingWork {
    running: Option<(u64, OwnedJob<PreparedEdit, WorkError>)>,
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

    pub(super) fn in_flight(&self) -> bool {
        self.running.is_some() || self.tracing.is_some()
    }

    /// Stop running work for close and drop what has not started.
    pub(super) fn cancel(&mut self) {
        if let Some((_, job)) = &self.running {
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

    /// The document with snapshot identity `identity`, shown or parked.
    fn editing_document(&self, identity: u64) -> Option<&Document> {
        let shown = self.document.scene.as_ref().map(|scene| &scene.document);
        shown.filter(|document| document.snapshot().identity() == identity).or_else(|| {
            self.inputs.parked_by_identity(identity)?.scene.as_ref().map(|scene| &scene.document)
        })
    }

    /// Start the oldest waiting deferred edit and the waiting selection trace when their slots
    /// are free. Saturated admission keeps them waiting for the completion wake.
    pub(super) fn submit_editing_work(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Ok(());
        }
        while self.editing.work.running.is_none()
            && let Some(mut work) = self.editing.work.waiting.pop_front()
        {
            let identity = work.base().identity();
            // Earlier work of this document may have landed since this was requested.
            match self.editing_document(identity).map(|document| work.rebase(document)) {
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
            let spec = self.spec(
                Operation::Infrastructure,
                JobClass::InfrastructureOrdinary,
                bytes,
                WORK_DEADLINE,
                None,
            )?;
            let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
            let mut slot = Some(work);
            match runtime.client.submit_blocking_owned_input(
                &runtime.scope,
                spec,
                &mut slot,
                |work, context| work.run(&context).map_err(WorkError::from),
            ) {
                Ok(job) => self.editing.work.running = Some((identity, job)),
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
            let spec = self.spec(
                Operation::Infrastructure,
                JobClass::InfrastructureOrdinary,
                bytes,
                WORK_DEADLINE,
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
            self.editing.work.running.as_mut().and_then(|(_, job)| job.try_take_completion())
            && let Some((identity, job)) = self.editing.work.running.take()
        {
            finished = true;
            Self::observe_terminal(&job, &completion.outcome);
            match Self::terminal(completion.outcome) {
                Ok(prepared) if !self.closing => self.complete_editing(identity, prepared),
                Ok(_) => {}
                Err(cause) => {
                    tracing::warn!(operation = "canvas_coverage_work", document_id = identity, cause = %cause, "Coverage work did not finish; the document is unchanged");
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
        if finished { self.submit_editing_work() } else { Ok(()) }
    }

    /// Apply a finished edit to the document it read, wherever that document is held.
    fn complete_editing(&mut self, identity: u64, prepared: PreparedEdit) {
        let now = self.navigation_time();
        let document = if self
            .document
            .scene
            .as_ref()
            .is_some_and(|scene| scene.document.snapshot().identity() == identity)
        {
            Some(&mut *self.document)
        } else {
            self.inputs.parked_mut(identity)
        };
        let Some(document) = document else {
            return;
        };
        let Some(scene) = document.scene.as_mut() else {
            return;
        };
        match document.tools.complete_deferred(prepared, &mut scene.document, now) {
            Ok(true) => {}
            Ok(false) => {
                tracing::debug!(
                    operation = "canvas_coverage_work",
                    document_id = identity,
                    "Finished coverage work changed nothing: it was stale, superseded or empty"
                );
            }
            Err(cause) => {
                tracing::warn!(operation = "canvas_coverage_work", document_id = identity, cause = %cause, "Finished coverage work could not be applied");
            }
        }
    }

    /// Stop editing work for close and release traced outlines, whose storage the scope's
    /// settlement waits for; running jobs stay observed until their terminal outcomes.
    pub(in crate::native_canvas::session) fn close_editing(&mut self) {
        let mut work = std::mem::take(&mut self.editing.work);
        work.cancel();
        self.editing = Editing { work, ..Editing::default() };
    }
}
