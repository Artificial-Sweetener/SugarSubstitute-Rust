// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep native GPU queue locks and driver calls on owned workers, with receipt-only UI handoff.
use super::*;
use sugarsubstitute_contracts::execution::Moment;
use sugarsubstitute_execution::OwnedInputError;

/// One validated frame awaiting admission, retaining its original deadline and native custody.
pub(super) struct PendingPresentation {
    pub(super) target: FrameStamp,
    pub(super) frame: WindowFrame,
    pub(super) deadline: Moment,
    pub(super) frame_period: Duration,
}

/// Keep frame identity paired with the admitted job through cancellation and actual retirement.
pub(super) struct Presenting {
    pub(super) target: FrameStamp,
    pub(super) job: OwnedJob<GpuPresentation, WorkError>,
}

impl PendingPresentation {
    /// The driver can block despite completed rendering. Only an admitted worker calls this.
    fn present(self, context: JobContext) -> Result<GpuPresentation, WorkError> {
        let started = context.now();
        let receipt = self.frame.present(&context)?;
        let elapsed = context.now().checked_duration_since(started).map_err(WorkError::new)?;
        if elapsed > self.frame_period {
            tracing::warn!(
                operation = "canvas_native_present_slow",
                generation = self.target.generation.get(),
                elapsed_microseconds = elapsed.as_micros(),
                frame_period_microseconds = self.frame_period.as_micros(),
                "Native GPU queue acceptance exceeded the display period on its worker"
            );
        }
        Ok(GpuPresentation { target: self.target, receipt })
    }
}

impl Session {
    /// The sole display slot spans preparation, native worker custody and desktop acknowledgement.
    pub(super) fn gpu_presentation_pending(&self) -> bool {
        #[cfg(all(windows, feature = "native-composition"))]
        let composition = self.composition_pending.is_some();
        #[cfg(not(all(windows, feature = "native-composition")))]
        let composition = false;
        self.pending_gpu_presentation.is_some()
            || self.presenting.is_some()
            || self.gpu_presentation.is_some()
            || self.awaiting_gpu.is_some()
            || composition
    }

    /// Transfer a validated frame only after admission, preserving it and its cutoff on rejection.
    pub(super) fn submit_gpu_presentation(&mut self) -> Result<(), Box<dyn Error>> {
        let superseded = self
            .document
            .scene
            .as_ref()
            .is_some_and(|scene| scene.source_revision != self.selection);
        if self.closing || self.pending_open.is_some() || superseded {
            if let Some(receipt) = self.gpu_presentation.take()
                && !self.closing
            {
                self.pacing.abandoned(receipt.target, self.navigation_time())?;
                self.document.pending_gpu_evidence = None;
            }
            if let Some(pending) = self.pending_gpu_presentation.take() {
                if !self.closing {
                    self.pacing.abandoned(pending.target, self.navigation_time())?;
                }
                self.document.pending_gpu_evidence = None;
                self.retire(Retired { _presentation: Some(pending.frame), ..Default::default() })?;
            }
            #[cfg(all(windows, feature = "native-composition"))]
            if !self.closing {
                self.abandon_composition_frame(self.navigation_time())?;
            }
            return Ok(());
        }
        if self.presenting.is_some()
            || self.gpu_presentation.is_some()
            || self.awaiting_gpu.is_some()
        {
            return Ok(());
        }
        let Some(pending) = &self.pending_gpu_presentation else {
            return Ok(());
        };
        let target = pending.target;
        let mut spec = self.spec(
            Operation::Infrastructure,
            JobClass::InfrastructureOrdinary,
            64 * 1024,
            Duration::from_secs(30),
            None,
        )?;
        spec.deadline = Some(pending.deadline);
        let runtime = self.runtime.as_ref().ok_or("Canvas execution is unavailable")?;
        match runtime.client.submit_blocking_owned_input(
            &runtime.scope,
            spec,
            &mut self.pending_gpu_presentation,
            PendingPresentation::present,
        ) {
            Ok(job) => self.presenting = Some(Presenting { target, job }),
            Err(OwnedInputError::Admission(cause)) if activity::retryable(&cause) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }

    /// Observe actual native return before releasing its display slot or proceeding to replacement.
    pub(super) fn consume_gpu_presentation(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(presenting) = self.presenting.as_mut() else {
            return Ok(());
        };
        let Some(completion) = presenting.job.try_take_completion() else {
            return Ok(());
        };
        let Presenting { target, job } =
            self.presenting.take().ok_or("Native presentation execution identity was lost")?;
        Self::observe_terminal(&job, &completion.outcome);
        let outcome = Self::terminal(completion.outcome);
        let superseded = self
            .document
            .scene
            .as_ref()
            .is_some_and(|scene| scene.source_revision != self.selection);
        if self.closing || self.pending_open.is_some() || superseded {
            if !self.closing {
                self.pacing.abandoned(target, self.navigation_time())?;
            }
            self.document.pending_gpu_evidence = None;
            if let Err(cause) = outcome {
                tracing::debug!(operation="canvas_native_present_stopped", cause=%cause,
                    "Native GPU presentation reached its terminal outcome during replacement");
            }
            return Ok(());
        }
        match outcome {
            Ok(receipt) => {
                if receipt.target != target
                    || self
                        .document
                        .pending_gpu_evidence
                        .is_none_or(|evidence| evidence.target != target)
                {
                    return Err("Canvas received a foreign native presentation receipt".into());
                }
                self.gpu_presentation = Some(receipt);
            }
            Err(cause) => {
                self.pacing.abandoned(target, self.navigation_time())?;
                self.document.pending_gpu_evidence = None;
                self.fail(cause);
            }
        }
        Ok(())
    }
}
