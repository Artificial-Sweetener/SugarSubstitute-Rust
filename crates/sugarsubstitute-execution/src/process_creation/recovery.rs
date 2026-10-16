// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Activate the actual pre-admitted profile creation once after its exact durable decision.
use super::*;
use crate::{admission::Core, scope::lock, task::Work};
use std::sync::{Arc, Weak};
use sugarsubstitute_contracts::{
    execution::{AdmissionError, CancelControl, JobCompletion, JobPhase, OwnedJob},
    incident::DurableAcknowledgement,
};
type CreationJob = OwnedJob<ProcessCreationReport, ProcessCreationFailure>;
/// Retain the real held native creation and its genuine once-only Root source.
/// Drop requests cancellation through OwnedJob. The queue/native owner destroys
/// the original factory and observes terminal; dropping is not native settlement.
#[must_use = "Activate or cancel and observe the actual held creation"]
pub struct RecoveryCreationPermit {
    job: CreationJob,
    core: Weak<Core>,
    grant: crate::RegisteredRecoveryGrant,
}
/// Preserve the identical held owner on a rejected first activation.
pub struct RecoveryCreationBeginRejection {
    /// Original actual clock, source, queued owner or acknowledgement mismatch.
    pub error: AdmissionError,
    /// The original permit, still responsible for cancellation/terminal observation.
    pub permit: RecoveryCreationPermit,
}
impl RecoveryCreationPermit {
    pub(super) fn new(
        job: CreationJob,
        core: &Arc<Core>,
        grant: crate::RegisteredRecoveryGrant,
    ) -> Self {
        Self { job, core: Arc::downgrade(core), grant }
    }
    /// Return matching facts of the genuine pre-admitted Root grant.
    pub fn key(&self) -> sugarsubstitute_contracts::recovery_supervisor::RecoveryGrantKey {
        self.grant.key()
    }
    /// Request cancellation without claiming destruction or output retirement.
    pub fn request_cancel(&self) {
        self.job.cancel();
    }
    /// Transfer the actual typed terminal if the held job stopped before activation.
    pub fn try_take_completion(
        &mut self,
    ) -> Option<JobCompletion<ProcessCreationReport, ProcessCreationFailure>> {
        self.job.try_take_completion()
    }
    /// Cancel and retain the same terminal observation owner for paired cleanup.
    pub fn cancel_and_observe(self) -> CreationJob {
        self.job.cancel();
        self.job
    }
    /// Release the exact original native factory after its persisted restart ACK.
    /// The composing Root owns durable commit and exact old family/profile retirement;
    /// this boundary validates the ACK's full report and next revision against the
    /// genuine grant, actual queued job and original finite admission. No new queue,
    /// job, clock interval, native source or byte reservation is introduced.
    /// # Errors
    /// Returns this identical permit if Root/source/current generation, actual held
    /// job, scope/cancellation/deadline or complete acknowledgement no longer matches.
    #[expect(
        clippy::result_large_err,
        reason = "Return the same pre-admitted 128 KiB creation owner and genuine grant inline on rejection"
    )]
    pub fn begin(
        self,
        acknowledgement: DurableAcknowledgement,
    ) -> Result<CreationJob, RecoveryCreationBeginRejection> {
        let decision = (|| {
            let core = self.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
            let now = core.clock.metadata().map_err(AdmissionError::Unexpected)?;
            let mut state = lock(&core.state);
            self.grant.state()?.validate_locked(&core, &state, now)?;
            let key = self.grant.key();
            if acknowledgement.incident() != key.report().incident()
                || key.report().revision().next().ok() != Some(acknowledgement.revision())
            {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            let id = self.job.spec().id;
            let record = state.jobs.get(&id).ok_or(AdmissionError::ShuttingDown)?;
            if record.scope_id != self.job.scope_id()
                || record.spec != self.job.spec()
                || record.phase != JobPhase::Queued
                || record.retiring
                || record.job.cancellation().is_requested()
            {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            let until = crate::admission_liveness::scope_deadline(&core, &state, record)?;
            if let Some(deadline) = until.filter(|until| *until <= now) {
                return Err(AdmissionError::DeadlineExpired { deadline, now });
            }
            let mut stages = lock(&self.grant.state()?.stages);
            let pipe = stages.pipe.and_then(|id| state.jobs.get(&id));
            if pipe.is_none_or(|record| {
                record.phase != JobPhase::Running
                    || record.retiring
                    || record.job.cancellation().is_requested()
                    || record.native_deadlines.is_none_or(|slots| slots.stopped())
            }) {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            if stages.creation != Some(id)
                || stages.pipe.is_none()
                || stages.closed
                || stages.committed
            {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            let queued = state
                .queue
                .iter_mut()
                .find(|queued| queued.context.job_id() == id)
                .ok_or(AdmissionError::InvalidCaptureAdmission)?;
            let Work::Native(crate::native::NativeWork::ProcessCreation(work, _)) =
                &mut queued.work
            else {
                return Err(AdmissionError::InvalidCaptureAdmission);
            };
            if !work.held {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            // Both mutations share Core's final fence; stopped creation cannot become runnable.
            work.held = false;
            stages.committed = true;
            drop(stages);
            drop(state);
            core.changed();
            Ok(())
        })();
        match decision {
            Ok(()) => Ok(self.job),
            Err(error) => Err(RecoveryCreationBeginRejection { error, permit: self }),
        }
    }
}

impl std::fmt::Debug for RecoveryCreationBeginRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryCreationBeginRejection { redacted }")
    }
}
impl std::fmt::Display for RecoveryCreationBeginRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Recovery creation activation rejected")
    }
}
impl std::error::Error for RecoveryCreationBeginRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
