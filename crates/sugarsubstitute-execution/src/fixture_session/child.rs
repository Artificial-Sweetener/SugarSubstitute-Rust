// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reserve before inherited ownership transfers; never rebase an admitted deadline.
use super::*;
use crate::{
    scope::lock,
    task::{ErasedJob, JobHandle},
};
use std::{
    error::Error,
    fmt,
    sync::atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_contracts::execution::{JobContext, Moment, OwnedJob};

/// Fixed absolute Windows QPC deadline, not an executor-local Moment or privilege.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixtureDeadline {
    ticks: u64,
    frequency: u64,
}
impl FixtureDeadline {
    /// Require positive signed-QPC-compatible values before native effects.
    pub fn new(ticks: u64, frequency: u64) -> Result<Self, FixtureChildAdmissionError> {
        if ticks == 0 || frequency == 0 || ticks > i64::MAX as u64 || frequency > i64::MAX as u64 {
            return Err(FixtureChildAdmissionError::InvalidSpecification);
        }
        Ok(Self { ticks, frequency })
    }
    /// Return the original absolute counter deadline in QPC ticks.
    pub const fn ticks(self) -> u64 {
        self.ticks
    }
    /// Return the enrolled counter frequency in ticks per second.
    pub const fn frequency(self) -> u64 {
        self.frequency
    }
}
/// Retain a reserve rejection without accepting any inherited native ownership.
#[derive(Debug)]
pub enum FixtureChildAdmissionError {
    /// The fixed deadline/input specification is invalid or already expired.
    InvalidSpecification,
    /// Actual QPC/QPF failed; original cause remains owned.
    Clock(NativeObservationError),
    /// Central admission rejected before bind; inherited input remains with caller.
    Admission(AdmissionError),
}
impl fmt::Display for FixtureChildAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Fixture child reservation rejected")
    }
}
impl Error for FixtureChildAdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Clock(error) => Some(error),
            Self::Admission(error) => Some(error),
            Self::InvalidSpecification => None,
        }
    }
}
/// Parsed four-role tokens retained by the isolated child until successful bind.
///
/// These numbers do not authenticate a peer or prove object type. Construction
/// performs no native operation and Drop closes nothing. Only pass handles which
/// the dedicated child invocation exclusively inherited. If reserve fails, that
/// invocation must fail and exit; its parent must observe actual OS death/family
/// settlement, without claiming clean child-side event retirement.
pub struct InheritedFixtureEvents {
    pub(crate) handles: [usize; 4],
}
impl InheritedFixtureEvents {
    /// Validate four distinct positive pointer-sized non-pseudo tokens in role order.
    pub fn new(
        ready: u64,
        release: u64,
        challenge: u64,
        response: u64,
    ) -> Result<Self, FixtureChildAdmissionError> {
        let values = [ready, release, challenge, response];
        let mut handles = [0; 4];
        for (index, value) in values.into_iter().enumerate() {
            let raw = usize::try_from(value)
                .map_err(|_| FixtureChildAdmissionError::InvalidSpecification)?;
            if raw == 0 || raw >= usize::MAX - 15 || handles[..index].contains(&raw) {
                return Err(FixtureChildAdmissionError::InvalidSpecification);
            }
            handles[index] = raw;
        }
        Ok(Self { handles })
    }
}
/// Actual child protocol effects after inherited handles and registrations close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixtureChildReport {
    /// Correlation supplied before ownership transfer.
    pub correlation: NativeCorrelation,
    /// All four inherit flags were cleared and verified before Ready.
    pub inheritance_cleared: bool,
    /// Native SetEvent(Ready) succeeded after both waits registered.
    pub ready_signalled: bool,
    /// Native SetEvent(Response) succeeded after actual Challenge callback.
    pub response_signalled: bool,
    /// Native Release callback was observed; this does not prove process exit.
    pub release_observed: bool,
}
pub(crate) trait ChildLease: Send {
    fn start(
        &mut self,
        challenge: Arc<Signal>,
        release: Arc<Signal>,
    ) -> Result<(), NativeObservationError>;
    fn ready(&mut self) -> Result<(), NativeObservationError>;
    fn response(&mut self) -> Result<(), NativeObservationError>;
    fn validate_observation(&self) -> Result<(), NativeObservationError>;
    fn retire(&mut self) -> Result<(), NativeObservationError>;
}
pub(crate) struct ChildBinding {
    lease: Mutex<Option<Box<dyn ChildLease>>>,
    ready: AtomicBool,
}
impl ChildBinding {
    pub(crate) fn ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
    pub(crate) fn take(&self) -> Option<Box<dyn ChildLease>> {
        lock(&self.lease).take()
    }
}
pub(crate) struct ChildWork {
    pub(crate) binding: Arc<ChildBinding>,
    pub(crate) job: Arc<Job<FixtureChildReport, NativeObservationError>>,
    pub(crate) correlation: NativeCorrelation,
}
/// Reserve deadline, memory and delivery before accepting four inherited handles.
/// Bind or cancel_unbound once; Drop requests unbound cancellation and abandons
/// delivery. Consume/drop this permit before the execution owner's final join.
#[must_use = "Bind or cancel the admitted fixture child reservation"]
pub struct FixtureChildPermit {
    job: Arc<Job<FixtureChildReport, NativeObservationError>>,
    deadline: Moment,
    binding: Arc<ChildBinding>,
    core: Arc<Core>,
    transferred: bool,
}
impl FixtureChildPermit {
    /// Read the original admitted deadline in this execution owner's monotonic nanoseconds.
    ///
    /// This immutable matching value permits finite pre-bind work to share the
    /// already translated parent budget. It performs no clock/native operation,
    /// transfers no ownership, and remains unchanged through cancellation or late
    /// binding. Bind or cancel the permit and consume its actual typed terminal
    /// before final execution teardown; the deadline is not settlement evidence.
    pub const fn deadline(&self) -> Moment {
        self.deadline
    }
    pub(crate) fn bind_lease(
        mut self,
        lease: Box<dyn ChildLease>,
    ) -> OwnedJob<FixtureChildReport, NativeObservationError> {
        *lock(&self.binding.lease) = Some(lease);
        self.binding.ready.store(true, Ordering::Release);
        self.transferred = true;
        self.core.changed();
        self.owned_job()
    }
    fn owned_job(&self) -> OwnedJob<FixtureChildReport, NativeObservationError> {
        OwnedJob::new(self.job.scope_id, self.job.spec, Box::new(JobHandle(self.job.clone())))
    }
    /// Retire unused reservation with actual typed delivery; accepts no inherited input.
    pub fn cancel_unbound(mut self) -> OwnedJob<FixtureChildReport, NativeObservationError> {
        self.job.cancel();
        self.binding.ready.store(true, Ordering::Release);
        self.transferred = true;
        self.core.changed();
        self.owned_job()
    }
}
impl Drop for FixtureChildPermit {
    fn drop(&mut self) {
        if !self.transferred {
            self.job.abandon();
            self.binding.ready.store(true, Ordering::Release);
            self.core.changed();
        }
    }
}
pub(crate) fn translated(
    local_before_qpc: Moment,
    observed_ticks: u64,
    observed_frequency: u64,
    deadline: FixtureDeadline,
) -> Result<Moment, FixtureChildAdmissionError> {
    if observed_ticks == 0
        || observed_frequency != deadline.frequency
        || observed_ticks >= deadline.ticks
    {
        return Err(FixtureChildAdmissionError::InvalidSpecification);
    }
    let nanos = u128::from(deadline.ticks - observed_ticks)
        .checked_mul(1_000_000_000)
        .ok_or(FixtureChildAdmissionError::InvalidSpecification)?
        / u128::from(observed_frequency);
    let nanos =
        u64::try_from(nanos).map_err(|_| FixtureChildAdmissionError::InvalidSpecification)?;
    local_before_qpc
        .checked_add(std::time::Duration::from_nanos(nanos))
        .map_err(|_| FixtureChildAdmissionError::InvalidSpecification)
}
pub(crate) fn reserve_child(
    core: &Arc<Core>,
    scope: &Scope,
    mut spec: JobSpec,
    correlation: NativeCorrelation,
    deadline: Moment,
) -> Result<FixtureChildPermit, FixtureChildAdmissionError> {
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
    spec.deadline = Some(deadline);
    core.admit(scope, spec, RequestKind::Native, |context: JobContext, cancel, admitted| {
        let job = Job::new(scope.id(), spec, admitted, cancel, Arc::downgrade(core));
        let binding =
            Arc::new(ChildBinding { lease: Mutex::new(None), ready: AtomicBool::new(false) });
        let work = ChildWork { binding: binding.clone(), job: job.clone(), correlation };
        let queued = QueuedJob {
            context,
            job: job.clone(),
            work: Work::Native(crate::native::NativeWork::FixtureChild(work)),
        };
        (
            queued,
            FixtureChildPermit { job, deadline, binding, core: core.clone(), transferred: false },
        )
    })
    .map_err(FixtureChildAdmissionError::Admission)
}
pub(crate) fn reserve_sampled(
    core: &Arc<Core>,
    scope: &Scope,
    spec: JobSpec,
    correlation: NativeCorrelation,
    deadline: FixtureDeadline,
    sample: impl FnOnce() -> Result<(u64, u64), FixtureChildAdmissionError>,
) -> Result<FixtureChildPermit, FixtureChildAdmissionError> {
    if spec.deadline.is_some() {
        return Err(FixtureChildAdmissionError::InvalidSpecification);
    }
    let local_before_qpc = core.clock.now();
    let (ticks, frequency) = sample()?;
    reserve_child(
        core,
        scope,
        spec,
        correlation,
        translated(local_before_qpc, ticks, frequency, deadline)?,
    )
}

#[cfg(test)]
#[path = "child_tests.rs"]
mod tests;
