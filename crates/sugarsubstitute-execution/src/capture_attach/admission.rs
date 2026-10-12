// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve source ownership on rejection and install native custody before dispatch.
use super::*;
use crate::{
    admission::RequestKind,
    task::{JobHandle, QueuedJob, Work},
};
use std::fmt;
use sugarsubstitute_contracts::execution::{
    AdmissionError, JobClass, JobSpec, Operation, OwnedJob, Scope,
};
use types::CaptureAttachExpectation;

/// Reject conflicting timing or class facts before any native operation.
pub enum CaptureAttachAdmissionError {
    /// Exact central admission rejection, with original capacity/identity facts.
    Admission(AdmissionError),
    /// Capture cleanup requires a persistent native scope with no ancestor deadline.
    ConflictingDeadline,
    /// Exact run already holds native/source custody in this execution owner.
    RunAlreadyReserved(sugarsubstitute_contracts::lifecycle::RunKey),
}
impl fmt::Debug for CaptureAttachAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => error.fmt(f),
            Self::ConflictingDeadline => f.write_str("ConflictingDeadline"),
            Self::RunAlreadyReserved(_) => f.write_str("RunAlreadyReserved { redacted }"),
        }
    }
}
impl fmt::Display for CaptureAttachAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Capture attachment admission rejected")
    }
}
impl std::error::Error for CaptureAttachAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::ConflictingDeadline | Self::RunAlreadyReserved(_) => None,
        }
    }
}
/// Return the exact untransferred source view and immutable expectation on rejection.
pub struct CaptureAttachAdmissionRejection {
    /// Original admission cause; no source/native acquisition has started.
    pub error: CaptureAttachAdmissionError,
    /// Original full run, identities, build and sixty-second budget.
    pub expected: CaptureAttachExpectation,
    /// Original source view; retaining it permits an unchanged valid admission retry.
    pub source: Arc<dyn CaptureEndpointSource>,
}
impl CaptureAttachAdmissionRejection {
    /// Return the exact reason, immutable expectation and identical untransferred source.
    /// Retry through the execution capture boundary with these unchanged facts.
    pub fn into_parts(
        self,
    ) -> (CaptureAttachAdmissionError, CaptureAttachExpectation, Arc<dyn CaptureEndpointSource>)
    {
        (self.error, self.expected, self.source)
    }
}
impl fmt::Debug for CaptureAttachAdmissionRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaptureAttachAdmissionRejection")
            .field("error", &self.error)
            .field("expected", &self.expected)
            .finish_non_exhaustive()
    }
}

pub(crate) fn reserve(
    core: &Arc<Core>,
    scope: &Scope,
    spec: JobSpec,
    expected: CaptureAttachExpectation,
    source: Arc<dyn CaptureEndpointSource>,
    lease: Box<dyn AttachLease>,
) -> Result<
    OwnedJob<CaptureAttachLaunchPin, CaptureAttachFailure>,
    Box<CaptureAttachAdmissionRejection>,
> {
    let reject = |error| {
        Box::new(CaptureAttachAdmissionRejection { error, expected, source: source.clone() })
    };
    if spec.deadline.is_some() {
        return Err(reject(CaptureAttachAdmissionError::ConflictingDeadline));
    }
    if spec.class != JobClass::NativeObservation
        || spec.operation != Operation::NativeLifecycle
        || spec.replacement.is_some()
    {
        return Err(reject(CaptureAttachAdmissionError::Admission(AdmissionError::WrongClass {
            operation: spec.operation,
            class: spec.class,
        })));
    }
    if spec.reservation_bytes < 16_384 {
        return Err(reject(CaptureAttachAdmissionError::Admission(
            AdmissionError::NativeReservationTooSmall {
                requested_bytes: spec.reservation_bytes,
                minimum_bytes: 16_384,
            },
        )));
    }
    let handshake = expected.attach_budget().until();
    // Validate actual immutable scope ancestry while preserving the original source.
    {
        let state = lock(&core.state);
        let stored = core
            .owned_scope(&state, scope, scope.owner_id())
            .map_err(|error| reject(CaptureAttachAdmissionError::Admission(error)))?;
        if stored.deadline.is_some() {
            return Err(reject(CaptureAttachAdmissionError::ConflictingDeadline));
        }
    }
    let now = core.clock.retirement_now();
    if core.clock.failed() {
        return Err(reject(CaptureAttachAdmissionError::Admission(AdmissionError::ShuttingDown)));
    }
    if handshake <= now {
        return Err(reject(CaptureAttachAdmissionError::Admission(
            AdmissionError::DeadlineExpired { deadline: handshake, now },
        )));
    }
    let retained = source.clone();
    core.admit(
        scope,
        spec,
        RequestKind::CaptureAttach(expected.run()),
        |context, cancel, admitted| {
            let job = Job::new(scope.id(), spec, admitted, cancel, Arc::downgrade(core));
            let custody = Custody::new(core, spec.id, expected);
            let shared = Arc::new(Shared {
                custody,
                state: Mutex::new(PinState {
                    pinned: false,
                    handles: None,
                    endpoint: None,
                    retirement: None,
                    retirement_published: false,
                }),
            });
            let queued = QueuedJob {
                context,
                job: job.clone(),
                work: Work::Native(crate::native::NativeWork::CaptureAttach(Box::new(
                    AttachWork { shared, source: retained, job: job.clone(), lease, handshake },
                ))),
            };
            (queued, OwnedJob::new(scope.id(), spec, Box::new(JobHandle(job))))
        },
    )
    .map_err(|error| {
        reject(match error {
            AdmissionError::ConflictingNativeRun(run) => {
                CaptureAttachAdmissionError::RunAlreadyReserved(run)
            }
            error => CaptureAttachAdmissionError::Admission(error),
        })
    })
}
