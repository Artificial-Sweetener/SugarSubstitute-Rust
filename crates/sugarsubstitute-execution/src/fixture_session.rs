// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own admitted fixture events, protocol commands and checked native retirement.
pub(crate) mod child;
pub(crate) mod child_worker;
mod control;
mod types;
pub(crate) mod worker;
pub use control::{FixtureChildEvents, FixtureSession, FixtureSessionControl};
pub use types::{
    FixtureObservation, FixtureSessionError, FixtureSessionReport, MIN_FIXTURE_SESSION_BYTES,
};

use crate::{
    admission::{Core, RequestKind},
    native::{NativeCorrelation, NativeObservationError, Signal},
    scope::lock,
    task::{Job, QueuedJob, Work},
};
use std::sync::{Arc, Mutex, Weak};
use sugarsubstitute_contracts::execution::{AdmissionError, JobSpec, Scope};

pub(crate) trait ParentLease: Send {
    fn start(
        &mut self,
        ready: Arc<Signal>,
        response: Arc<Signal>,
    ) -> Result<[usize; 4], NativeObservationError>;
    fn challenge(&mut self) -> Result<(), NativeObservationError>;
    fn validate_observation(&self) -> Result<(), NativeObservationError>;
    fn release(&mut self) -> Result<(), NativeObservationError>;
    fn retire(&mut self) -> Result<(), NativeObservationError>;
}
#[derive(Default)]
struct State {
    prepared: bool,
    ready: bool,
    responded: bool,
    challenge_requested: bool,
    challenge_committed: bool,
    challenge_sent: bool,
    release_requested: bool,
    release_committed: bool,
    release_sent: bool,
    stopped: bool,
    transfer_taken: bool,
    pin: bool,
    handles: [usize; 4],
    unread: [bool; 3],
}
pub(crate) struct Shared {
    state: Mutex<State>,
    core: Weak<Core>,
}
impl Shared {
    fn changed(&self) {
        if let Some(core) = self.core.upgrade() {
            core.changed();
        }
    }
}
pub(crate) struct FixtureWork {
    pub(crate) shared: Arc<Shared>,
    pub(crate) job: Arc<Job<FixtureSessionReport, NativeObservationError>>,
    pub(crate) correlation: NativeCorrelation,
    pub(crate) lease: Option<Box<dyn ParentLease>>,
}
pub(crate) fn reserve(
    core: &Arc<Core>,
    scope: &Scope,
    spec: JobSpec,
    correlation: NativeCorrelation,
    lease: Box<dyn ParentLease>,
) -> Result<FixtureSession, AdmissionError> {
    if spec.reservation_bytes < MIN_FIXTURE_SESSION_BYTES {
        return Err(AdmissionError::NativeReservationTooSmall {
            requested_bytes: spec.reservation_bytes,
            minimum_bytes: MIN_FIXTURE_SESSION_BYTES,
        });
    }
    if spec.deadline.is_none() {
        return Err(AdmissionError::DeadlineRequired);
    }
    core.admit(scope, spec, RequestKind::Native, |context, cancel, admitted| {
        let job = Job::new(scope.id(), spec, admitted, cancel, Arc::downgrade(core));
        let shared =
            Arc::new(Shared { state: Mutex::new(State::default()), core: Arc::downgrade(core) });
        let queued = QueuedJob {
            context,
            job: job.clone(),
            work: Work::Native(crate::native::NativeWork::FixtureParent(FixtureWork {
                shared: shared.clone(),
                job: job.clone(),
                correlation,
                lease: Some(lease),
            })),
        };
        let session = FixtureSession::new(scope.id(), spec, shared, job);
        (queued, session)
    })
}
fn state(shared: &Shared) -> std::sync::MutexGuard<'_, State> {
    lock(&shared.state)
}

#[cfg(test)]
mod fifo_tests;
#[cfg(test)]
mod tests;
