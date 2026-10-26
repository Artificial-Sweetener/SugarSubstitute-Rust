// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit coalesced viewer work and owned cleanup through the existing execution boundary.
use super::*;
use sugarsubstitute_execution::OwnedInputError;

impl Session {
    /// Drain metadata and typed completions before submitting the latest application intent.
    pub(super) fn advance(&mut self) -> Result<(), Box<dyn Error>> {
        self.tools.advance(self.navigation_time())?;
        self.cancel_obsolete_refinement()?;
        if let Some(runtime) = &self.runtime {
            let batch =
                runtime.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            if batch.notifications.iter().any(|notice| notice.scope_id != runtime.scope.id()) {
                return Err("Canvas received a foreign execution scope notice".into());
            }
        }
        self.consume_gpu_presentation()?;
        self.consume_initialization()?;
        if self.pending_retirement.is_none() {
            self.consume_device()?;
        }
        if self.pending_retirement.is_none() {
            self.consume_load()?;
        }
        if self.pending_retirement.is_none() {
            self.consume_frame()?;
        }
        if self.pending_retirement.is_none() {
            self.consume_display()?;
        }
        if self.pending_retirement.is_none() {
            self.submit_gpu_presentation()?;
        }
        if let Some(outcome) = Self::take(&mut self.retirement) {
            if let Err(cause) = Self::terminal(outcome) {
                self.fail(cause);
            }
            if self.pending_retirement.is_none() {
                self.retirement_deadline = None;
            }
        }
        if self.closing {
            self.advance_close()?;
        }
        self.submit_retirement()?;
        if self.closing || self.pending_retirement.is_some() || self.retirement.is_some() {
            return Ok(());
        }
        if self.initialization_pending {
            self.submit_initialization()?;
        }
        if self.initializing.is_some() || self.resources.is_none() {
            return Ok(());
        }
        if self.picker {
            return Ok(());
        }
        if !self.gpu_ready {
            if self.failed.is_none() {
                self.initialize_gpu()?;
            }
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
        if self.failed.is_none() {
            self.submit_display()?;
        }
        if self.loading.is_none() && self.rendering.is_none() && self.failed.is_none() {
            if self.gpu_presentation_pending() {
                return Ok(());
            }
            self.submit_frame()?;
        }
        Ok(())
    }
    /// Fund and transfer disposal metadata only after actual admission; never drop source handles
    /// or large prepared payloads on the UI thread merely because a newer selection superseded them.
    fn submit_retirement(&mut self) -> Result<(), Box<dyn Error>> {
        if self.retirement.is_some() || self.pending_retirement.is_none() {
            return Ok(());
        }
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        let mut spec = self.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureLifecycle,
            64 * 1024,
            Duration::from_secs(5),
            None,
        )?;
        spec.deadline = self.retirement_deadline;
        match runtime.client.submit_blocking_owned_box(
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
        if self.initializing.is_some()
            || self.device_job.is_some()
            || self.loading.is_some()
            || self.rendering.is_some()
            || self.displaying.is_some()
            || self.presenting.is_some()
            || self.pending_retirement.is_some()
            || self.retirement.is_some()
        {
            return Ok(());
        }
        if self.resources.is_some()
            || self.scene.is_some()
            || self.front.is_some()
            || self.fallback_front.is_some()
            || self.gpu_front.is_some()
            || self.fallback_gpu_front.is_some()
            || self.navigation_front.is_some()
            || self.navigation_gpu_front.is_some()
            || self.gpu_presentation.is_some()
            || self.pending_gpu_presentation.is_some()
            || self.cpu_projection.is_some()
            || self.display_buffers.iter().any(Option::is_some)
        {
            let retired = Retired {
                _scene: self.scene.take(),
                _resources: self.resources.take(),
                _front: self.front.take(),
                _fallback_front: self.fallback_front.take(),
                _gpu_front: self.gpu_front.take(),
                _fallback_gpu_front: self.fallback_gpu_front.take(),
                _navigation_front: self.navigation_front.take(),
                _navigation_gpu_front: self.navigation_gpu_front.take(),
                _presentation: self
                    .pending_gpu_presentation
                    .take()
                    .map(|presentation| presentation.frame),
                _projection: self.cpu_projection.take(),
                _buffers: std::mem::take(&mut self.display_buffers),
                ..Default::default()
            };
            self.presented = None;
            self.fallback_presented = None;
            self.navigation_presented = None;
            self.display_evidence = None;
            self.display_buffer_extent = None;
            self.projection_base = None;
            self.gpu_presentation = None;
            self.retire(retired)?;
        } else if !self.shutdown_started {
            if let Some(runtime) = &mut self.runtime {
                runtime
                    .owner
                    .request_shutdown(self.clock.now().checked_add(Duration::from_secs(5))?);
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
