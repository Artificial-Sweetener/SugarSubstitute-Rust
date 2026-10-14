// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit one fixed custody pool on the existing native worker without losing rejected owners.
use super::*;
use crate::{
    admission::RequestKind,
    task::{Job, JobHandle, QueuedJob, Work},
};
use std::{fmt, sync::Arc};
use sugarsubstitute_contracts::execution::{
    AdmissionError, JobClass, JobSpec, NativeResourceError, Operation, OwnedJob, Scope,
};

/// Exact original pool: two MiB metadata and 128 existing 512-KiB slots.
pub const BACKEND_CUSTODY_POOL_BYTES: u64 = 66 * 1024 * 1024;

/// Reject a worker before transferring the complete original child-session permit.
#[derive(Debug)]
pub enum BackendCustodyWorkerAdmissionError {
    /// Actual transport/provider rejection, preserving its original source.
    Session(crate::native::NativeObservationError),
    /// Actual queue, memory, identity or scope rejection.
    Admission(AdmissionError),
    /// The fixed worker pool or persistent native scope was not requested.
    Specification,
}
impl fmt::Display for BackendCustodyWorkerAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Backend custody worker admission rejected")
    }
}
impl std::error::Error for BackendCustodyWorkerAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Session(cause) => Some(cause),
            Self::Admission(cause) => Some(cause),
            Self::Specification => None,
        }
    }
}

impl crate::ExecutionClient {
    /// Reserve the fixed original custody pool before allocating worker controls.
    /// The returned start funds durable source construction before driver binding.
    /// The driver must adopt outputs before effects in `initialize`. Actual child-session
    /// identity is rechecked under the same Core fence as admission. Rejection
    /// leaves the identical permit in `input`. No executor is created.
    /// # Errors
    /// Rejects foreign/stopped sessions, invalid fixed pool, queue/memory or scope.
    pub fn reserve_backend_custody_worker(
        &self,
        scope: &Scope,
        spec: JobSpec,
        input: &mut Option<BackendCustodyWorkerPermit>,
    ) -> Result<BackendCustodyWorkerStart, BackendCustodyWorkerAdmissionError> {
        if spec.operation != Operation::NativeLifecycle
            || spec.class != JobClass::NativeObservation
            || spec.reservation_bytes != BACKEND_CUSTODY_POOL_BYTES
            || spec.deadline.is_some()
            || spec.replacement.is_some()
            || scope.spec().deadline.is_some()
        {
            return Err(BackendCustodyWorkerAdmissionError::Specification);
        }
        let permit = input.as_ref().ok_or(BackendCustodyWorkerAdmissionError::Specification)?;
        if let Err(cause) = permit.validate_current_admission() {
            return Err(BackendCustodyWorkerAdmissionError::Session(cause));
        }
        let source = permit.shared.clone();
        let original = permit.owner.clone();
        let kind = RequestKind::BackendCustody { source: &source, original: &original };
        self.core
            .admit_owned(scope, spec, kind, input, |input, context, cancel, timing| {
                let Some(permit) = input.as_ref() else {
                    return Err((AdmissionError::ShuttingDown, input));
                };
                let key = permit.key();
                let observed_peer = { crate::scope::lock(&permit.shared.state).authenticated_peer };
                let peer = match observed_peer {
                    Some(peer) => peer,
                    None => {
                        return Err((
                            AdmissionError::NativeResources(
                                NativeResourceError::OriginalAdmissionRetired,
                            ),
                            input,
                        ));
                    }
                };
                let claim = match context.memory().claim(2 * 1024 * 1024) {
                    Ok(claim) => claim,
                    Err(cause) => return Err((AdmissionError::Memory(cause), input)),
                };
                let permit = match input.take() {
                    Some(permit) => permit,
                    None => return Err((AdmissionError::ShuttingDown, input)),
                };
                let metadata = BackendCustodyMemory::new(
                    claim,
                    context.clone(),
                    key,
                    (peer.pid, peer.birth),
                    Arc::downgrade(&self.core),
                    permit.shared.clone(),
                    permit.owner.clone(),
                );
                let job =
                    Job::new(scope.id(), spec, timing.admitted, cancel, Arc::downgrade(&self.core));
                let binding = super::start::Binding::new(metadata.clone());
                let work =
                    Box::new(worker::Work { binding: binding.clone(), permit, job: job.clone() });
                let queued = QueuedJob {
                    context,
                    job: job.clone(),
                    work: Work::Native(crate::native::NativeWork::BackendCustody(work, metadata)),
                };
                Ok((
                    queued,
                    BackendCustodyWorkerStart {
                        job: Some(OwnedJob::new(scope.id(), spec, Box::new(JobHandle(job)))),
                        binding,
                    },
                ))
            })
            .map_err(|(cause, _)| BackendCustodyWorkerAdmissionError::Admission(cause))
    }
}

/// Validate the actual originating child allocation with the caller's Core guard.
pub(crate) fn validate_locked(
    core: &Arc<crate::admission::Core>,
    state: &crate::admission::State,
    source: &crate::lifecycle_pipe::SharedRef,
    original: &crate::lifecycle_pipe::Owner,
    now: sugarsubstitute_contracts::execution::Moment,
) -> Result<(), AdmissionError> {
    if !source.child || !source.core.ptr_eq(&Arc::downgrade(core)) {
        return Err(AdmissionError::NativeResources(NativeResourceError::OriginalAdmissionRetired));
    }
    crate::lifecycle_pipe::current_admission::source_locked(core, state, source, original, now)
}
