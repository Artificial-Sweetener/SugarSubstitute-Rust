// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover exclusive fonts before separately admitting a completed installer frame.
use super::*;
use crate::render_transfer::TransferError;
use sugarsubstitute_contracts::execution::JobOutcome;

impl Driver {
    /// Observe the original admitted identity; displayed pixels never supply job authority.
    pub(super) fn take_terminal(&mut self) -> Result<Option<Prepared>> {
        let Some(active) = &mut self.active else { return Ok(None) };
        let Some(completion) = active.job.try_take_completion() else { return Ok(None) };
        let job = active.job.spec().id;
        let scope = active.job.scope_id();
        let identity = active.request.as_ref().map(|request| request.identity);
        self.active = None;
        if completion.spec.id != job || completion.scope_id != scope {
            return Err("Installer render completion does not match its admission".into());
        }
        tracing::debug!(operation = "installer_preparation", job_id = job.get(),
            queue_ns = completion.queue_time.as_nanos(), execution_ns = completion.execution_time.as_nanos(),
            outcome = ?completion.outcome.kind(), "Observed installer render completion");
        match completion.outcome {
            JobOutcome::Success(Output { renderer, frame }) => {
                self.inputs = Some(renderer.inputs.clone());
                self.renderer = Some(PreparationTransfer::new(renderer));
                let Some(frame) = frame else {
                    if identity.is_some() {
                        return Err("Installer frame returned initialization".into());
                    }
                    return Ok(None);
                };
                let prepared = frame?;
                if Some(prepared.frame.identity()) != identity {
                    return Err("Installer pixels do not match their admitted request".into());
                }
                if self.gate.accept(&prepared.frame) == FramePublicationDecision::Accepted {
                    Ok(Some(prepared))
                } else {
                    Ok(None)
                }
            }
            JobOutcome::Failure(error) => Err(error.into()),
            JobOutcome::Panicked(cause) => Err(io::Error::other(cause.as_str().to_owned()).into()),
            JobOutcome::Cancelled | JobOutcome::TimedOut | JobOutcome::Coalesced { .. } => {
                if let Some(transfer) = self.renderer.take() {
                    match transfer.take() {
                        Ok(renderer) => self.renderer = Some(PreparationTransfer::new(renderer)),
                        Err(TransferError::Consumed) => self.inputs = None,
                        Err(error) => return Err(error.into()),
                    }
                }
                // Retry current state, never a consumed or superseded snapshot.
                self.pending = None;
                self.dirty = true;
                Ok(None)
            }
        }
    }
}
