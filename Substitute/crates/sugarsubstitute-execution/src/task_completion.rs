// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Construct and install typed completion under the one actual Core terminal fence.
//!
//! Ordinary jobs hand their worker-side context to [`Job::finish_at`], which drops it before the
//! terminal outcome is installed: once a consumer can observe the outcome, the reservation is held
//! only by the completion, the result and guards the job itself retained. Native custody workers
//! publish through [`Job::finish_in_custody_at`] and keep their context, and with it the
//! reservation, until custody settles.

use super::*;
use std::time::Duration;
use sugarsubstitute_contracts::execution::{
    AdmissionError, ExecutionError, ExecutionId, MemoryLease, MemoryRelease, NativeHandleLease,
};

/// Release port of a completion that carries no reservation.
struct Unreserved;

impl MemoryRelease for Unreserved {
    fn release(&self, _bytes: u64) {}
}

impl<T: Send + 'static, E: Send + 'static> Job<T, E> {
    /// Build the completion's timing and outcome around the guards it retains.
    fn completion(
        &self,
        started: Option<Moment>,
        ended: Moment,
        outcome: JobOutcome<T, E>,
        memory: MemoryLease,
        native_handles: Option<NativeHandleLease>,
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
            memory,
            native_handles,
        }
    }

    /// Build the original owned completion before taking its final admission fence.
    /// No supplied provider, native call or final outcome destruction crosses Core.
    fn completion_at(
        &self,
        context: &JobContext,
        started: Option<Moment>,
        ended: Moment,
        outcome: JobOutcome<T, E>,
    ) -> JobCompletion<T, E> {
        self.completion(
            started,
            ended,
            outcome,
            context.memory().clone(),
            context.native_handles().cloned(),
        )
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
    fn completion_delivered(&self, core: &Core, owner: ExecutionId, wake: bool) {
        if wake {
            core.mailbox.deliver_wake();
        }
        core.changed();
        tracing::debug!(operation = ?self.spec.operation, execution_id = owner.get(), scope_id = self.scope_id.get(), job_id = self.spec.id.get(), "Owned work terminal installed");
    }

    /// Install a built completion, or retain the installation failure for the owner.
    fn publish(&self, completion: JobCompletion<T, E>, owner: ExecutionId) {
        let mut completion = Some(completion);
        if let Some(core) = self.core.upgrade() {
            match self.install_completion(&core, &mut completion, |_| Ok(())) {
                Ok(wake) => self.completion_delivered(&core, owner, wake),
                Err(cause) => {
                    core.mailbox
                        .record_failure(ExecutionError::new(self.spec.operation, Box::new(cause)));
                    core.mailbox.deliver_wake();
                }
            }
        }
    }

    /// Publish an ordinary job's terminal outcome at the already observed owner clock sample.
    /// The worker-side `context` is dropped before installation, so a consumer that observes the
    /// outcome and drops the completion and result also observes the reservation released.
    pub(crate) fn finish_at(
        &self,
        context: JobContext,
        started: Option<Moment>,
        ended: Moment,
        outcome: JobOutcome<T, E>,
    ) {
        let completion = self.completion_at(&context, started, ended, outcome);
        let owner = context.owner_id();
        drop(context);
        self.publish(completion, owner);
    }

    /// Publish a native custody job's terminal outcome while its worker keeps `context` and its
    /// custody guards. The reservation is released when that custody settles, not at publication.
    pub(crate) fn finish_in_custody_at(
        &self,
        context: &JobContext,
        started: Option<Moment>,
        ended: Moment,
        outcome: JobOutcome<T, E>,
    ) {
        self.publish(self.completion_at(context, started, ended, outcome), context.owner_id());
    }

    /// Publish a panic that unwound the job's own future. Unwinding already dropped that future's
    /// context, so the completion carries no reservation and no native handle accounting.
    pub(crate) fn finish_unwound(&self, started: Moment, cause: PanicCause) {
        let Some(core) = self.core.upgrade() else {
            return;
        };
        let ended = core.clock.retirement_now();
        let unreserved = MemoryLease::new(0, Box::new(Unreserved));
        let completion =
            self.completion(Some(started), ended, JobOutcome::Panicked(cause), unreserved, None);
        self.publish(completion, core.id);
    }

    /// Validate fresh application eligibility under the SAME terminal install fence.
    /// The validator observes actual current Core state and takes only documented
    /// subordinate locks. It invokes no provider/native call or outcome destructor.
    /// Rejection returns the identical original outcome after all locks leave. Native custody
    /// publishes this way, so the worker keeps `context` and its reservation until custody settles.
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
                    self.completion_delivered(&core, context.owner_id(), wake);
                    return Ok(true);
                }
                Err(cause) => cause,
            },
            None => AdmissionError::ShuttingDown,
        };
        Err((accepted, completion.map(|original| original.outcome)))
    }
}
