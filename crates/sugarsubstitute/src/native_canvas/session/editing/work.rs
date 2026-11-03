// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run deferred coverage work and selection tracing as owned blocking jobs, one of each at a
//! time, beside the frame pipeline's CPU lanes.
//!
//! A tool's newest request replaces one still waiting to start. A finished edit applies to the
//! document it read, shown or parked, and only while that document still shows the revision it
//! read; anything else is stale and changes nothing.
use super::*;
use sugarsubstitute_canvas::{BoundaryWork, CoverageBoundary, DeferredEdit, PreparedEdit};
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

/// The running and waiting coverage work and the running selection trace.
#[derive(Default)]
pub(super) struct EditingWork {
    running: Option<(u64, OwnedJob<PreparedEdit, WorkError>)>,
    queued: Option<DeferredEdit>,
    tracing: Option<([u8; 32], OwnedJob<CoverageBoundary, WorkError>)>,
    trace: Option<BoundaryWork>,
    /// Working storage the waiting trace reserves.
    trace_bytes: u64,
}

impl EditingWork {
    /// Keep the newest deferred request until a job can start it.
    pub(super) fn queue(&mut self, work: DeferredEdit) {
        self.queued = Some(work);
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
        self.queued = None;
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

    /// Start the waiting deferred edit and selection trace when their slots are free.
    /// Saturated admission keeps them waiting for the completion wake.
    pub(super) fn submit_editing_work(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Ok(());
        }
        if self.editing.work.running.is_none()
            && let Some(work) = &self.editing.work.queued
        {
            let identity = work.base().identity();
            let bytes = Self::work_bytes(work.base().extent(), WORK_BYTES_PER_PIXEL)?;
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
                &mut self.editing.work.queued,
                |work, context| work.run(&context).map_err(WorkError::from),
            ) {
                Ok(job) => self.editing.work.running = Some((identity, job)),
                Err(OwnedInputError::Admission(cause)) if activity::retryable(&cause) => {}
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

    /// Observe finished coverage work and traces, applying what is still current.
    pub(in crate::native_canvas::session) fn consume_editing_work(
        &mut self,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(completion) =
            self.editing.work.running.as_mut().and_then(|(_, job)| job.try_take_completion())
            && let Some((identity, job)) = self.editing.work.running.take()
        {
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
        Ok(())
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
        if let Err(cause) = document.tools.complete_deferred(prepared, &mut scene.document, now) {
            tracing::warn!(operation = "canvas_coverage_work", document_id = identity, cause = %cause, "Finished coverage work could not be applied");
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
