// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Atomically transfer native lease inputs only after actual central pipe admission.
use super::*;
use sugarsubstitute_contracts::execution::MemoryClaim;

/// Retain constructed native storage before its outside original metadata claim.
/// Captured sessions install this claim before any native owner allocation.
pub(crate) struct PreparedPipeLease {
    pub(crate) lease: Box<dyn PipeLease>,
    pub(crate) metadata: Option<MemoryClaim>,
}

pub(crate) struct PipeAdmission<T: Send + 'static> {
    pub(crate) binding: Option<PeerBinding>,
    pub(crate) handshake: Moment,
    pub(crate) owner: fn(Arc<Job<T, LifecyclePipeFailure>>) -> Owner,
}

pub(crate) fn admit<T: Send + 'static>(
    core: &Arc<Core>,
    scope: &Scope,
    spec: JobSpec,
    binding: Option<PeerBinding>,
    handshake: Moment,
    lease: Box<dyn PipeLease>,
    owner: fn(Arc<Job<T, LifecyclePipeFailure>>) -> Owner,
) -> Result<(Control, OwnedJob<T, LifecyclePipeFailure>), AdmissionError> {
    admit_owned(
        core,
        scope,
        spec,
        PipeAdmission { binding, handshake, owner },
        RequestKind::Native,
        lease,
        |lease, _context| Ok(PreparedPipeLease { lease, metadata: None }),
    )
    .map_err(|(error, _lease)| error)
}

pub(crate) fn admit_owned<T: Send + 'static, M>(
    core: &Arc<Core>,
    scope: &Scope,
    spec: JobSpec,
    admission: PipeAdmission<T>,
    kind: RequestKind<'_>,
    input: M,
    lease: impl FnOnce(M, &JobContext) -> Result<PreparedPipeLease, (AdmissionError, M)>,
) -> Result<(Control, OwnedJob<T, LifecyclePipeFailure>), (AdmissionError, M)> {
    if spec.reservation_bytes < MIN_LIFECYCLE_PIPE_BYTES {
        return Err((
            AdmissionError::NativeReservationTooSmall {
                requested_bytes: spec.reservation_bytes,
                minimum_bytes: MIN_LIFECYCLE_PIPE_BYTES,
            },
            input,
        ));
    }
    let now = match core.clock.metadata() {
        Ok(now) => now,
        Err(cause) => return Err((AdmissionError::Unexpected(cause), input)),
    };
    if admission.handshake <= now {
        return Err((
            AdmissionError::DeadlineExpired { deadline: admission.handshake, now },
            input,
        ));
    }
    let captured = matches!(&kind, RequestKind::CapturedLifecycle(_));
    core.admit_owned(scope, spec, kind, input, |input, context, cancel, timing| {
        let admitted = timing.admitted;
        let handshake = timing
            .native_handshake
            .map_or(admission.handshake, |until| until.min(admission.handshake));
        let prepared = lease(input, &context)?;
        let metadata = metadata::Metadata::new(prepared.metadata);
        let job = Job::new(scope.id(), spec, admitted, cancel, Arc::downgrade(core));
        let shared = SharedRef::new(
            Shared {
                child: admission.binding.is_none(),
                captured,
                original_deadline: handshake,
                state: Mutex::new(State::new(admission.binding)),
                core: Arc::downgrade(core),
                id: spec.id,
            },
            metadata,
        );
        let retained = (admission.owner)(job.clone());
        let queued = QueuedJob {
            context,
            job: job.clone(),
            work: Work::Native(crate::native::NativeWork::LifecyclePipe(PipeWork {
                shared: shared.clone(),
                owner: retained.clone(),
                lease: Some(prepared.lease),
            })),
        };
        Ok((
            queued,
            (
                Control { shared, owner: retained },
                OwnedJob::new(scope.id(), spec, Box::new(JobHandle(job))),
            ),
        ))
    })
}
