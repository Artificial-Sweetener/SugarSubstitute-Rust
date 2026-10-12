// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one actual cleanup transaction through sampling, delivery and native retirement.
use crate::{admission::Core, native::NativeObservationError, scope::lock};
use std::sync::{Arc, Mutex, Weak};
use sugarsubstitute_contracts::{
    execution::{ExecutionError, ExecutionId, JobId, MemoryClaim, Moment, Operation},
    lifecycle::{RunKey, ShutdownBudget},
};

/// Checked native counter facts for one original cleanup transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureCleanupCounterBudget {
    started: i64,
    graceful: i64,
    total: i64,
    frequency: i64,
}
impl CaptureCleanupCounterBudget {
    #[cfg(windows)]
    fn from_total(
        total: sugarsubstitute_contracts::execution::CounterDeadline,
    ) -> Result<Self, NativeObservationError> {
        let frequency = total.frequency();
        let fifteen = frequency.checked_mul(15).ok_or_else(counter_overflow)?;
        let ten = frequency.checked_mul(10).ok_or_else(counter_overflow)?;
        let started = total
            .ticks()
            .checked_sub(fifteen)
            .filter(|value| *value > 0)
            .ok_or_else(counter_overflow)?;
        let graceful = started.checked_add(ten).ok_or_else(counter_overflow)?;
        Ok(Self { started, graceful, total: total.ticks(), frequency })
    }
    /// Return conservative original native start ticks, never a new clock observation.
    pub const fn started_counter(self) -> i64 {
        self.started
    }
    /// Return the exact native ten-second graceful deadline.
    pub const fn graceful_counter(self) -> i64 {
        self.graceful
    }
    /// Return the exact native fifteen-second total deadline.
    pub const fn total_counter(self) -> i64 {
        self.total
    }
    /// Return original positive native counter ticks per second.
    pub const fn frequency(self) -> i64 {
        self.frequency
    }
}
#[cfg(windows)]
fn counter_overflow() -> NativeObservationError {
    NativeObservationError::new(
        "capture_cleanup_counter_arithmetic",
        sugarsubstitute_contracts::execution::TimeError::Overflow,
    )
}

/// Immutable cleanup facts, separate from actual native closure or process settlement.
#[derive(Clone, Debug)]
pub enum CaptureCleanupSnapshot {
    /// Healthy preparation/session has never requested cleanup.
    Dormant,
    /// The single first claimant is observing the real clock outside owner locks.
    Requested,
    /// Original execution budget remains immutable even when native encoding fails.
    Latched {
        /// Actual first-claim execution-clock ten/fifteen-second budget.
        budget: ShutdownBudget,
        /// Original native tuple or its original conversion failure; no retry renewal.
        counter: Result<CaptureCleanupCounterBudget, NativeObservationError>,
    },
    /// The first observation failed before any valid execution budget existed.
    ClockUnavailable(NativeObservationError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CleanupTrigger {
    FailedPreparation,
    Cancelled,
    Released,
    ScopeShutdown,
    OwnerShutdown,
}
pub(crate) struct CleanupState {
    pub(crate) snapshot: CaptureCleanupSnapshot,
    pub(crate) trigger: Option<CleanupTrigger>,
    pub(crate) stopped: bool,
    pub(crate) native_retired: bool,
    pub(crate) source_retired: bool,
    pub(crate) active: bool,
    sampling: bool,
    pub(crate) memory: Option<MemoryClaim>,
}
impl CleanupState {
    /// Observe retained physical/source custody and in-flight sampling under its existing guard.
    /// This passive predicate neither acquires a lock nor invokes an external provider.
    pub(crate) fn held(&self) -> bool {
        !self.native_retired || !self.source_retired || self.sampling
    }
}

pub(crate) struct Custody {
    pub(crate) full_run: RunKey,
    pub(crate) attach_until: Moment,
    pub(crate) execution: ExecutionId,
    pub(crate) job: JobId,
    pub(crate) core: Weak<Core>,
    pub(crate) state: Mutex<CleanupState>,
}
impl Custody {
    /// Retain exact local deadline facts without reconstructing an attachment duration.
    /// Parent uses its original budget; child uses the authoritative counter conversion.
    pub(crate) fn new(
        core: &Arc<Core>,
        job: JobId,
        full_run: RunKey,
        attach_until: Moment,
    ) -> Arc<Self> {
        Arc::new(Self {
            full_run,
            attach_until,
            execution: core.id,
            job,
            core: Arc::downgrade(core),
            state: Mutex::new(CleanupState {
                snapshot: CaptureCleanupSnapshot::Dormant,
                trigger: None,
                stopped: false,
                native_retired: false,
                source_retired: false,
                active: false,
                sampling: false,
                memory: None,
            }),
        })
    }
    pub(crate) fn held(&self) -> bool {
        lock(&self.state).held()
    }
    pub(crate) fn active(&self) -> bool {
        let state = lock(&self.state);
        state.active && (!state.native_retired || !state.source_retired || state.sampling)
    }
    pub(crate) fn snapshot(&self) -> CaptureCleanupSnapshot {
        lock(&self.state).snapshot.clone()
    }
    pub(crate) fn stopped(&self) -> bool {
        lock(&self.state).stopped
    }
    pub(crate) fn release_memory(&self) {
        let memory = {
            let mut state = lock(&self.state);
            if state.native_retired && state.source_retired && !state.sampling {
                state.memory.take()
            } else {
                None
            }
        };
        drop(memory);
    }
}

/// Stop one authenticated inherited capture lifetime without exposing native handles.
/// The actual execution/input producer alone constructs this non-Clone control.
pub struct CaptureCleanupControl {
    pub(crate) custody: Arc<Custody>,
}
impl CaptureCleanupControl {
    /// Request the first cleanup transaction; repeated calls return its original facts.
    /// The fallible provider is called outside internal locks by only the first claimant.
    /// Requested reports a concurrent observation and never waits or spins.
    pub fn request_cleanup(&self) -> CaptureCleanupSnapshot {
        request(&self.custody, CleanupTrigger::Released)
    }
    /// Read bounded facts without observing a provider or renewing deadlines.
    pub fn snapshot(&self) -> CaptureCleanupSnapshot {
        self.custody.snapshot()
    }
}

struct CleanupClaim {
    custody: Arc<Custody>,
    core: Arc<Core>,
}
pub(crate) fn request(custody: &Arc<Custody>, trigger: CleanupTrigger) -> CaptureCleanupSnapshot {
    let Some(core) = custody.core.upgrade() else {
        let mut cleanup = lock(&custody.state);
        if matches!(cleanup.snapshot, CaptureCleanupSnapshot::Dormant) {
            cleanup.stopped = true;
            cleanup.trigger = Some(trigger);
            cleanup.snapshot =
                CaptureCleanupSnapshot::ClockUnavailable(NativeObservationError::new(
                    "capture_cleanup_owner_unavailable",
                    std::io::Error::other("Actual execution owner is unavailable"),
                ));
        }
        return cleanup.snapshot.clone();
    };
    let claim = {
        let state = lock(&core.state);
        let mut cleanup = lock(&custody.state);
        if !matches!(cleanup.snapshot, CaptureCleanupSnapshot::Dormant) {
            return cleanup.snapshot.clone();
        }
        cleanup.stopped = true;
        cleanup.trigger = Some(trigger);
        cleanup.snapshot = CaptureCleanupSnapshot::Requested;
        cleanup.sampling = true;
        let claim = CleanupClaim { custody: custody.clone(), core: core.clone() };
        drop(cleanup);
        drop(state);
        claim
    };
    core.changed();
    let observation = core
        .clock
        .metadata()
        .map_err(|cause| NativeObservationError::new("capture_cleanup_clock", cause))
        .and_then(|started| {
            ShutdownBudget::new(started)
                .map_err(|cause| NativeObservationError::new("capture_cleanup_budget", cause))
        });
    let snapshot = match observation {
        Ok(budget) => CaptureCleanupSnapshot::Latched { budget, counter: encode(&core, budget) },
        Err(cause) => CaptureCleanupSnapshot::ClockUnavailable(cause),
    };
    {
        let mut state = lock(&claim.core.state);
        let mut cleanup = lock(&claim.custody.state);
        cleanup.snapshot = snapshot.clone();
        if let CaptureCleanupSnapshot::Latched { budget, .. } = snapshot
            && let Some(record) = state.jobs.get_mut(&claim.custody.job)
            && let Some(deadlines) = &mut record.native_deadlines
        {
            deadlines.latch_capture_cleanup(budget);
        }
    }
    // Actual retirement/acknowledgment cannot release custody until the original
    // failure has transferred to the owner's bounded channel. The strong claim
    // owns this exact state even if the native worker finished during sampling.
    let failure = match &snapshot {
        CaptureCleanupSnapshot::Latched { counter: Err(cause), .. }
        | CaptureCleanupSnapshot::ClockUnavailable(cause) => Some(cause.clone()),
        _ => None,
    };
    if let Some(cause) = &failure {
        claim.core.mailbox.record_failure(ExecutionError::new(
            Operation::NativeLifecycle,
            Box::new(cause.clone()),
        ));
    }
    let acknowledged = {
        let state = lock(&claim.core.state);
        lock(&claim.custody.state).sampling = false;
        state.jobs.get(&claim.custody.job).is_some_and(|record| record.acknowledged)
    };
    claim.custody.release_memory();
    if acknowledged {
        claim.core.acknowledge(claim.custody.job);
    }
    claim.core.changed();
    if failure.is_some() {
        claim.core.mailbox.deliver_wake();
    }
    snapshot
}

fn encode(
    core: &Arc<Core>,
    budget: ShutdownBudget,
) -> Result<CaptureCleanupCounterBudget, NativeObservationError> {
    #[cfg(windows)]
    {
        crate::ExecutionClient { core: core.clone() }
            .encode_counter_deadline(budget.settle_until)
            .and_then(CaptureCleanupCounterBudget::from_total)
    }
    #[cfg(not(windows))]
    {
        let _ = (core, budget);
        Err(NativeObservationError::new(
            "capture_cleanup_counter",
            std::io::Error::other("Native capture counter is unavailable on this platform"),
        ))
    }
}

/// Constructor-private proof of actual capture pin and temporary I/O retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureAttachRetirementReport {
    pub(crate) run: RunKey,
    pub(crate) execution: ExecutionId,
    pub(crate) job: JobId,
}
impl CaptureAttachRetirementReport {
    /// Return the complete desktop lifetime whose actual custody retired.
    pub const fn run(self) -> RunKey {
        self.run
    }
    /// Return the exact executor associated with that native lifetime.
    pub const fn execution_id(self) -> ExecutionId {
        self.execution
    }
    /// Return the original admitted job identity, never a new retirement job.
    pub const fn job_id(self) -> JobId {
        self.job
    }
}
