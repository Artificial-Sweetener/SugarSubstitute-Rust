// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Construct and install typed completion under the one actual Core terminal fence.

use super::*;
use std::time::Duration;
use sugarsubstitute_contracts::execution::{AdmissionError, ExecutionError};

impl<T: Send + 'static, E: Send + 'static> Job<T, E> {
    /// Build the original owned completion before taking its final admission fence.
    /// No supplied provider, native call or final outcome destruction crosses Core.
    fn completion_at(
        &self,
        context: &JobContext,
        started: Option<Moment>,
        ended: Moment,
        outcome: JobOutcome<T, E>,
    ) -> JobCompletion<T, E> {
        let start = started.unwrap_or(ended);
        let duration = |later: Moment, earlier: Moment| -> Duration {
            match later.checked_duration_since(earlier) {
                Ok(duration) => duration,
                Err(error) => {
                    tracing::error!(operation = ?self.spec.operation, job_id = self.spec.id.get(), cause = %error, "Injected execution clock regressed");
                    Duration::ZERO
                }
            }
        };
        JobCompletion {
            scope_id: self.scope_id,
            spec: self.spec,
            queue_time: duration(start, self.admitted),
            execution_time: duration(ended, start),
            outcome,
            memory: context.memory().clone(),
            native_handles: context.native_handles().cloned(),
        }
    }

    /// Install only this original completion after its exact passive validator.
    /// The caller keeps the completion outside Core on validator rejection.
    fn install_completion(
        &self,
        core: &Core,
        completion: &mut Option<JobCompletion<T, E>>,
        validate: impl FnOnce(&mut crate::admission::State) -> Result<(), AdmissionError>,
    ) -> Result<bool, AdmissionError> {
        let owned = completion.as_ref().ok_or(AdmissionError::ShuttingDown)?;
        let kind = owned.outcome.kind();
        let replacement = match &owned.outcome {
            JobOutcome::Coalesced { replacement } => Some(*replacement),
            _ => None,
        };
        core.terminal(self.spec.id, kind, replacement, validate, || {
            let mut cell = lock(&self.cell);
            cell.phase = JobPhase::Terminal(kind);
            cell.completion = completion.take();
            !cell.abandoned
        })
    }

    /// Deliver notification only after terminal installation released Core.
    fn completion_delivered(&self, core: &Core, context: &JobContext, wake: bool) {
        if wake {
            core.mailbox.deliver_wake();
        }
        core.changed();
        tracing::debug!(operation = ?self.spec.operation, execution_id = context.owner_id().get(), scope_id = self.scope_id.get(), job_id = self.spec.id.get(), "Owned work terminal installed");
    }

    /// Publish actual stop using the already observed original owner clock sample.
    pub(crate) fn finish_at(
        &self,
        context: &JobContext,
        started: Option<Moment>,
        ended: Moment,
        outcome: JobOutcome<T, E>,
    ) {
        let mut completion = Some(self.completion_at(context, started, ended, outcome));
        if let Some(core) = self.core.upgrade() {
            match self.install_completion(&core, &mut completion, |_| Ok(())) {
                Ok(wake) => self.completion_delivered(&core, context, wake),
                Err(cause) => {
                    core.mailbox
                        .record_failure(ExecutionError::new(self.spec.operation, Box::new(cause)));
                    core.mailbox.deliver_wake();
                }
            }
        }
    }

    /// Validate fresh application eligibility under the SAME terminal install fence.
    /// The validator observes actual current Core state and takes only documented
    /// subordinate locks. It invokes no provider/native call or outcome destructor.
    /// Rejection returns the identical original outcome after all locks leave.
    pub(crate) fn finish_current_at(
        &self,
        context: &JobContext,
        started: Moment,
        ended: Moment,
        outcome: JobOutcome<T, E>,
        validate: impl FnOnce(&mut crate::admission::State) -> Result<(), AdmissionError>,
    ) -> Result<bool, (AdmissionError, Option<JobOutcome<T, E>>)> {
        let mut completion = Some(self.completion_at(context, Some(started), ended, outcome));
        let accepted = match self.core.upgrade() {
            Some(core) => match self.install_completion(&core, &mut completion, validate) {
                Ok(wake) => {
                    if let Some(original) = completion {
                        return Err((
                            AdmissionError::InvalidCaptureAdmission,
                            Some(original.outcome),
                        ));
                    }
                    self.completion_delivered(&core, context, wake);
                    return Ok(true);
                }
                Err(cause) => cause,
            },
            None => AdmissionError::ShuttingDown,
        };
        Err((accepted, completion.map(|original| original.outcome)))
    }
}
