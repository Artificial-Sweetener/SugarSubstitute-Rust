// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Atomically transfer native lease inputs only after actual central pipe admission.
use super::*;

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
        |lease| lease,
    )
    .map_err(|(error, _lease)| error)
}

pub(crate) fn admit_owned<T: Send + 'static, M>(
    core: &Arc<Core>,
    scope: &Scope,
    spec: JobSpec,
    admission: PipeAdmission<T>,
    kind: RequestKind,
    input: M,
    lease: impl FnOnce(M) -> Box<dyn PipeLease>,
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
    core.admit_owned(scope, spec, kind, input, |input, context, cancel, admitted| {
        let job = Job::new(scope.id(), spec, admitted, cancel, Arc::downgrade(core));
        let shared = Arc::new(Shared {
            original_deadline: admission.handshake,
            state: Mutex::new(State::new(admission.binding)),
            memory: Mutex::new(None),
            core: Arc::downgrade(core),
            id: spec.id,
        });
        let retained = (admission.owner)(job.clone());
        let queued = QueuedJob {
            context,
            job: job.clone(),
            work: Work::Native(crate::native::NativeWork::LifecyclePipe(PipeWork {
                shared: shared.clone(),
                owner: retained.clone(),
                lease: Some(lease(input)),
                handshake: admission.handshake,
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
