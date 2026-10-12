// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind acknowledged terminal jobs to their captured publication identity.

use super::*;
use sugarsubstitute_contracts::execution::JobOutcome;

impl PreparationDriver {
    /// Observe actual termination before restoring cache ownership or accepting pixels.
    pub(super) fn take_terminal(&mut self) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
        let Some(active) = &mut self.active else { return Ok(None) };
        let Some(completion) = active.job.try_take_completion() else { return Ok(None) };
        let expected_job = active.job.spec().id;
        let expected_scope = active.job.scope_id();
        let expected_identity = active.identity;
        let bootstrap = active.bootstrap;
        self.active = None;
        if completion.spec.id != expected_job || completion.scope_id != expected_scope {
            return Err("Preparation completion belongs to a different admitted job".into());
        }
        tracing::debug!(operation = "splash_preparation", job_id = expected_job.get(),
            queue_ns = completion.queue_time.as_nanos(), execution_ns = completion.execution_time.as_nanos(),
            outcome = ?completion.outcome.kind(), reserved_bytes = completion.memory.bytes(),
            "Observed preparation termination");
        match completion.outcome {
            JobOutcome::Success(PreparationOutput {
                renderer,
                frame: PreparationFrame::Initialized,
                console,
                console_clipped_rows,
            }) => {
                if console.is_some() || console_clipped_rows != 0 {
                    return Err("Initialization returned a frame input".into());
                }
                if expected_identity.is_some() {
                    return Err("Frame job returned initialization".into());
                }
                self.renderer = Some(PreparationTransfer::new(renderer));
                Ok(None)
            }
            JobOutcome::Success(PreparationOutput {
                renderer,
                frame: PreparationFrame::Prepared(frame),
                console,
                console_clipped_rows,
            }) => {
                tracing::debug!(
                    operation = "splash_preparation",
                    job_id = expected_job.get(),
                    console_clipped_rows,
                    "Observed projected console clipping"
                );
                self.renderer = Some(PreparationTransfer::new(renderer));
                let frame = frame?;
                if Some(frame.identity()) != expected_identity {
                    return Err("Prepared pixels do not match their admitted request".into());
                }
                let decision = self.gate.accept(&frame);
                drop(console);
                if decision == FramePublicationDecision::Accepted {
                    self.bootstrap_published = true;
                    self.enriched |= !bootstrap;
                    Ok(Some(frame))
                } else {
                    Ok(None)
                }
            }
            JobOutcome::Failure(error) => Err(error.into()),
            JobOutcome::Panicked(cause) => {
                Err(std::io::Error::other(cause.as_str().to_owned()).into())
            }
            JobOutcome::Cancelled | JobOutcome::TimedOut | JobOutcome::Coalesced { .. } => {
                if self.closing {
                    return Ok(None);
                }
                if let Some(transfer) = self.renderer.take() {
                    match transfer.take() {
                        Ok(renderer) => self.renderer = Some(PreparationTransfer::new(renderer)),
                        Err(crate::render_transfer::TransferError::Consumed) => {}
                        Err(error) => return Err(error.into()),
                    }
                }
                Ok(None)
            }
        }
    }
}
