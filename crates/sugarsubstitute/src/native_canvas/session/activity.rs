// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit coalesced viewer work and owned cleanup through the existing execution boundary.
use super::*;
use sugarsubstitute_execution::OwnedInputError;

impl Session {
    /// Consume typed completions before submitting the latest application intent.
    pub(super) fn advance(&mut self) -> Result<(), Box<dyn Error>> {
        #[cfg(feature = "native-composition")]
        self.consume_preview_poll_request();
        self.document.tools.advance(self.navigation_time())?;
        self.cancel_obsolete_refinement()?;
        if let Some(runtime) = &self.runtime {
            runtime.drain_standalone()?;
        }
        self.consume_gpu_presentation()?;
        if self.deferred_retirement.is_none() {
            self.consume_initialization()?;
        }
        if self.deferred_retirement.is_none() {
            self.consume_device()?;
        }
        #[cfg(all(windows, feature = "native-composition"))]
        if self.deferred_retirement.is_none() {
            self.consume_host_graphics()?;
        }
        #[cfg(feature = "native-composition")]
        self.consume_drag_out()?;
        #[cfg(feature = "native-composition")]
        let restoring_preview = self.preview_restoring();
        #[cfg(not(feature = "native-composition"))]
        let restoring_preview = false;
        if self.deferred_retirement.is_none() && !restoring_preview {
            self.consume_load()?;
        }
        #[cfg(feature = "native-composition")]
        self.consume_input_job()?;
        #[cfg(feature = "native-composition")]
        self.consume_input_capture()?;
        #[cfg(feature = "native-composition")]
        self.consume_input_persistence()?;
        #[cfg(feature = "native-composition")]
        self.advance_acquirer_retirement()?;
        if self.deferred_retirement.is_none() {
            self.consume_frame()?;
        }
        if self.deferred_retirement.is_none() {
            self.consume_display()?;
        }
        if self.deferred_retirement.is_none() {
            self.submit_gpu_presentation()?;
        }
        if let Some(outcome) = Self::take(&mut self.retirement) {
            if let Err(cause) = Self::terminal(outcome) {
                self.fail(cause);
            }
            if self.pending_retirement.is_none() {
                self.pending_retirement = self.deferred_retirement.take();
                self.retirement_deadline = self.deferred_retirement_deadline.take();
            }
        }
        #[cfg(feature = "native-composition")]
        if self.deferred_retirement.is_none() {
            self.advance_preview_resize()?;
        }
        #[cfg(feature = "native-composition")]
        self.advance_preview()?;
        if self.closing {
            self.advance_close()?;
        }
        #[cfg(feature = "native-composition")]
        let restoring = self.advance_document_transaction()? || self.preview_restoring();
        #[cfg(not(feature = "native-composition"))]
        let restoring = false;
        #[cfg(all(windows, feature = "native-composition"))]
        self.advance_video_memory();
        #[cfg(feature = "native-composition")]
        self.advance_retained_retirement()?;
        #[cfg(feature = "native-composition")]
        self.advance_input_retirement()?;
        self.submit_retirement()?;
        if restoring
            || self.closing
            || self.pending_retirement.is_some()
            || self.deferred_retirement.is_some()
            || self.retirement.is_some()
        {
            return Ok(());
        }
        #[cfg(feature = "native-composition")]
        if self.acquisition_retirement.is_some() {
            return Ok(());
        }
        if self.initialization_pending {
            self.submit_initialization()?;
        }
        if self.initializing.is_some() || self.resources.is_none() {
            return Ok(());
        }
        #[cfg(all(windows, feature = "native-composition"))]
        if self.host_graphics_pending {
            self.submit_host_graphics()?;
        }
        #[cfg(feature = "native-composition")]
        self.submit_input_job()?;
        #[cfg(feature = "native-composition")]
        self.submit_input_capture()?;
        #[cfg(feature = "native-composition")]
        self.submit_input_restore()?;
        #[cfg(feature = "native-composition")]
        self.advance_input_autosave()?;
        if self.picker {
            return Ok(());
        }
        if !self.gpu_ready {
            if self.failed.is_none() {
                self.initialize_gpu()?;
            }
            return Ok(());
        }
        #[cfg(feature = "native-composition")]
        if self.advance_input_route()? || self.advance_retained_restore()? {
            return Ok(());
        }
        if self.pending_open.is_some() {
            if self.loading.is_none()
                && self.rendering.is_none()
                && !self.gpu_presentation_pending()
            {
                self.submit_load()?;
            }
            return Ok(());
        }
        #[cfg(feature = "native-composition")]
        if self.document_suspended {
            return Ok(());
        }
        if self.document.scene.as_ref().is_none_or(|scene| scene.source_revision != self.selection)
        {
            return Ok(());
        }
        if self.failed.is_none() {
            self.submit_display()?;
        }
        if self.loading.is_none() && self.rendering.is_none() && self.failed.is_none() {
            #[cfg(feature = "native-composition")]
            if self.preview_shown() {
                return Ok(());
            }
            if self.gpu_presentation_pending() {
                return Ok(());
            }
            self.submit_frame()?;
        }
        Ok(())
    }
    /// Retain cleanup input even when deadline arithmetic fails before worker admission.
    pub(super) fn retire(&mut self, value: Retired) -> Result<(), Box<dyn Error>> {
        let cutoff = self.retirement_cutoff();
        let deadline = cutoff.as_ref().ok().copied();
        if self.pending_retirement.is_none() {
            self.pending_retirement = Some(value);
            self.retirement_deadline = deadline;
        } else if self.deferred_retirement.is_none() {
            self.deferred_retirement = Some(value);
            self.deferred_retirement_deadline = deadline;
        } else {
            return Err("Canvas retirement input is already retained".into());
        }
        cutoff.map(|_| ())
    }
    /// Bound disposal by the original close cutoff when shutdown is underway.
    pub(super) fn retirement_cutoff(&self) -> Result<Moment, Box<dyn Error>> {
        let deadline = self.clock.now().checked_add(Duration::from_secs(5))?;
        Ok(self.shutdown_deadline.map_or(deadline, |cutoff| deadline.min(cutoff)))
    }
    /// Transfer disposal through actual queue/completion admission while its payload's original
    /// claims continue funding retained storage; never drop native handles on the UI thread.
    fn submit_retirement(&mut self) -> Result<(), Box<dyn Error>> {
        if self.retirement.is_some() || self.pending_retirement.is_none() {
            return Ok(());
        }
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let mut spec = self.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureLifecycle,
            0,
            Duration::from_secs(5),
            None,
        )?;
        spec.deadline = self.retirement_deadline;
        match runtime.client.submit_blocking_owned_input(
            &runtime.scope,
            spec,
            &mut self.pending_retirement,
            |value, _| {
                drop(value);
                Ok::<(), WorkError>(())
            },
        ) {
            Ok(job) => self.retirement = Some(job),
            Err(OwnedInputError::Admission(cause)) if retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
    /// Dispose retained state while cleanup admission is still open, then ask execution to stop.
    fn advance_close(&mut self) -> Result<(), Box<dyn Error>> {
        #[cfg(all(windows, feature = "native-composition"))]
        let graphics_in_flight = self.host_graphics_job.is_some();
        #[cfg(not(all(windows, feature = "native-composition")))]
        let graphics_in_flight = false;
        #[cfg(feature = "native-composition")]
        let drag_out_in_flight = self.drag_out.in_flight();
        #[cfg(not(feature = "native-composition"))]
        let drag_out_in_flight = false;
        #[cfg(all(windows, feature = "native-composition"))]
        let graphics_retained = self.host_graphics.is_some();
        #[cfg(not(all(windows, feature = "native-composition")))]
        let graphics_retained = false;
        #[cfg(feature = "native-composition")]
        let acquisition_pending = self.acquisition_retirement.is_some() || self.inputs.in_flight();
        #[cfg(not(feature = "native-composition"))]
        let acquisition_pending = false;
        #[cfg(feature = "native-composition")]
        let preview_pending = self.preview_pending();
        #[cfg(not(feature = "native-composition"))]
        let preview_pending = false;
        if self.initializing.is_some()
            || self.device_job.is_some()
            || graphics_in_flight
            || drag_out_in_flight
            || self.loading.is_some()
            || self.rendering.is_some()
            || self.displaying.is_some()
            || self.presenting.is_some()
            || self.pending_retirement.is_some()
            || self.deferred_retirement.is_some()
            || self.retirement.is_some()
            || acquisition_pending
            || preview_pending
        {
            return Ok(());
        }
        #[cfg(all(windows, feature = "native-composition"))]
        if self.host_graphics.as_ref().is_some_and(|graphics| Arc::strong_count(graphics) != 1) {
            return Ok(());
        }
        #[cfg(all(windows, feature = "native-composition"))]
        let composition_retained = self.composition_device.is_some()
            || self.composition_pending.is_some()
            || self.composition_shown.is_some();
        #[cfg(not(all(windows, feature = "native-composition")))]
        let composition_retained = false;
        #[cfg(feature = "native-composition")]
        let documents_retained = self.retained.has_resources() || self.inputs.has_resources();
        #[cfg(not(feature = "native-composition"))]
        let documents_retained = false;
        if self.pending_open.is_some()
            || self.resources.is_some()
            || self.document.has_resources()
            || documents_retained
            || self.gpu_presentation.is_some()
            || self.pending_gpu_presentation.is_some()
            || graphics_retained
            || composition_retained
        {
            let cutoff = self.retirement_cutoff()?;
            let document = self.withdraw_document()?;
            let retired = Retired {
                _open: self.pending_open.take(),
                _document: Some(document),
                #[cfg(feature = "native-composition")]
                _retained_document: self.take_retained_document(),
                #[cfg(feature = "native-composition")]
                _retained: self.retained.take_all(),
                #[cfg(feature = "native-composition")]
                _inputs: self.take_input_documents(),
                _resources: self.resources.take(),
                #[cfg(all(windows, feature = "native-composition"))]
                _host_graphics: self.host_graphics.take(),
                _presentation: self
                    .pending_gpu_presentation
                    .take()
                    .map(|presentation| presentation.frame),
                #[cfg(all(windows, feature = "native-composition"))]
                _device: self.composition_device.take(),
                #[cfg(all(windows, feature = "native-composition"))]
                _composition: [self.composition_pending.take(), self.composition_shown.take()],
                ..Default::default()
            };
            self.gpu_presentation = None;
            // Diagnostics hold a claim on the retired reservation; observations end with it.
            self.gpu_diagnostics = None;
            self.retirement_deadline = Some(cutoff);
            self.pending_retirement = Some(retired);
        } else if !self.shutdown_started {
            let deadline = self.shutdown_deadline.ok_or("Canvas shutdown deadline absent")?;
            if let Some(runtime) = &mut self.runtime {
                runtime.request_shutdown(deadline);
            }
            if let Some(owner) = &mut self.starting_owner {
                owner.request_shutdown(deadline);
            }
            self.shutdown_started = true;
        }
        Ok(())
    }
}
/// Actual releases notify this execution owner's completion wake when a feasible retry can run.
pub(super) fn retryable(cause: &AdmissionError) -> bool {
    matches!(
        cause,
        AdmissionError::QueueSaturated { .. }
            | AdmissionError::MemorySaturated { .. }
            | AdmissionError::CompletionSaturated
    )
}
