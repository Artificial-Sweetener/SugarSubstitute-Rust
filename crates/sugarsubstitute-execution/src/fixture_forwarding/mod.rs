// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Forward one inherited event allowlist without signals, waits or child authority.
mod control;
pub(crate) mod worker;
use crate::fixture_session::{
    MIN_FIXTURE_SESSION_BYTES,
    child::{FixtureChildAdmissionError, FixtureDeadline},
};
use crate::{
    admission::{Core, RequestKind},
    native::{NativeCorrelation, NativeObservationError},
    scope::lock,
    task::{Job, QueuedJob, Work},
};
pub use control::FixtureForwardingControl;
pub use control::{FixtureForwardingEvents, FixtureForwardingPermit, FixtureForwardingSession};
use std::sync::{Arc, Mutex, Weak};
use sugarsubstitute_contracts::execution::{AdmissionError, JobContext, JobSpec, Moment, Scope};

/// Actual forwarding validation and closure; no field establishes process death.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixtureForwardingReport {
    /// Correlation enrolled before adopting inherited handles.
    pub correlation: NativeCorrelation,
    /// Original inherit flags were cleared and restricted duplicates prepared.
    pub prepared: bool,
}
pub(crate) trait ForwardLease: Send {
    fn prepare(&mut self) -> Result<[usize; 4], NativeObservationError>;
    fn retire(&mut self) -> Result<(), NativeObservationError>;
}
#[derive(Default)]
struct State {
    bound: bool,
    prepared: bool,
    transferred: bool,
    pinned: bool,
    stopped: bool,
    handles: [usize; 4],
    lease: Option<Box<dyn ForwardLease>>,
}
struct Shared {
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
pub(crate) struct ForwardWork {
    shared: Arc<Shared>,
    job: Arc<Job<FixtureForwardingReport, NativeObservationError>>,
    correlation: NativeCorrelation,
}
impl ForwardWork {
    pub(crate) fn ready(&self) -> bool {
        lock(&self.shared.state).bound
    }
}
pub(crate) fn reserve(
    core: &Arc<Core>,
    scope: &Scope,
    mut spec: JobSpec,
    correlation: NativeCorrelation,
    deadline: FixtureDeadline,
    sample: impl FnOnce() -> Result<(u64, u64), FixtureChildAdmissionError>,
) -> Result<FixtureForwardingPermit, FixtureChildAdmissionError> {
    if spec.deadline.is_some() {
        return Err(FixtureChildAdmissionError::InvalidSpecification);
    }
    if spec.reservation_bytes < MIN_FIXTURE_SESSION_BYTES {
        return Err(FixtureChildAdmissionError::Admission(
            AdmissionError::NativeReservationTooSmall {
                requested_bytes: spec.reservation_bytes,
                minimum_bytes: MIN_FIXTURE_SESSION_BYTES,
            },
        ));
    }
    let before = core.clock.now();
    let (ticks, frequency) = sample()?;
    spec.deadline =
        Some(crate::fixture_session::child::translated(before, ticks, frequency, deadline)?);
    reserve_at(core, scope, spec, correlation)
}
fn reserve_at(
    core: &Arc<Core>,
    scope: &Scope,
    spec: JobSpec,
    correlation: NativeCorrelation,
) -> Result<FixtureForwardingPermit, FixtureChildAdmissionError> {
    core.admit(scope, spec, RequestKind::Native, |context: JobContext, cancel, timing| {
        let admitted = timing.admitted;
        let job = Job::new(scope.id(), spec, admitted, cancel, Arc::downgrade(core));
        let shared =
            Arc::new(Shared { state: Mutex::new(State::default()), core: Arc::downgrade(core) });
        let queued = QueuedJob {
            context,
            job: job.clone(),
            work: Work::Native(crate::native::NativeWork::FixtureForward(ForwardWork {
                shared: shared.clone(),
                job: job.clone(),
                correlation,
            })),
        };
        (queued, FixtureForwardingPermit::new(shared, job))
    })
    .map_err(FixtureChildAdmissionError::Admission)
}

#[cfg(test)]
mod tests;
