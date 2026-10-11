// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Queue fixed commands and pin launch duplicates without closing handles on callers.
use super::*;
use crate::task::{ErasedJob, JobHandle};
use sugarsubstitute_contracts::execution::{CancelControl, JobSpec, OwnedJob, ScopeId};

/// Own one parent fixture operation and its independently observed terminal result.
#[must_use = "Retain session control and observe its actual terminal retirement"]
pub struct FixtureSession {
    /// Queue bounded commands and transfer the one launch pin.
    pub control: FixtureSessionControl,
    /// Terminal delivery occurs only after actual callback and parent-handle retirement.
    pub completion: OwnedJob<FixtureSessionReport, NativeObservationError>,
}
impl FixtureSession {
    pub(super) fn new(
        id: ScopeId,
        spec: JobSpec,
        shared: Arc<Shared>,
        job: Arc<Job<FixtureSessionReport, NativeObservationError>>,
    ) -> Self {
        Self {
            control: FixtureSessionControl { shared, job: job.clone() },
            completion: OwnedJob::new(id, spec, Box::new(JobHandle(job))),
        }
    }
}
/// Queue one Challenge and one Release; no method performs a native wait or signal.
/// Dropping control requests cancellation, which never releases or kills the child.
pub struct FixtureSessionControl {
    shared: Arc<Shared>,
    job: Arc<Job<FixtureSessionReport, NativeObservationError>>,
}
impl FixtureSessionControl {
    /// Transfer the single launch pin after actual preparation; cancellation closes transfer.
    /// `None` means preparation is not yet available. Taking it twice is rejected.
    pub fn take_child_events(&self) -> Result<Option<FixtureChildEvents>, FixtureSessionError> {
        let mut s = state(&self.shared);
        if s.stopped || self.job.cancel.is_requested() {
            return Err(FixtureSessionError::Closed);
        }
        if s.transfer_taken {
            return Err(FixtureSessionError::AlreadyRequested);
        }
        if !s.prepared {
            return Ok(None);
        }
        s.transfer_taken = true;
        s.pin = true;
        Ok(Some(FixtureChildEvents { shared: self.shared.clone() }))
    }
    /// Transfer one of three fixed actual facts; facts are independent of terminal notices.
    pub fn take_observation(&self) -> Option<FixtureObservation> {
        let mut s = state(&self.shared);
        let index = s.unread.iter().position(|flag| *flag)?;
        s.unread[index] = false;
        Some(
            [
                FixtureObservation::Prepared,
                FixtureObservation::Ready,
                FixtureObservation::Responded,
            ][index],
        )
    }
    /// Request the sole native Challenge after actual Ready; duplicates are rejected.
    pub fn request_challenge(&self) -> Result<(), FixtureSessionError> {
        let mut s = state(&self.shared);
        if s.stopped || s.release_requested || self.job.cancel.is_requested() {
            return Err(FixtureSessionError::Closed);
        }
        if !s.ready {
            return Err(FixtureSessionError::NotReady);
        }
        if s.challenge_requested {
            return Err(FixtureSessionError::AlreadyRequested);
        }
        s.challenge_requested = true;
        drop(s);
        self.shared.changed();
        Ok(())
    }
    /// Request explicit Release once after actual Ready, without implying process exit.
    pub fn request_release(&self) -> Result<(), FixtureSessionError> {
        let mut s = state(&self.shared);
        if s.stopped || self.job.cancel.is_requested() {
            return Err(FixtureSessionError::Closed);
        }
        if !s.ready {
            return Err(FixtureSessionError::NotReady);
        }
        if s.release_requested {
            return Err(FixtureSessionError::AlreadyRequested);
        }
        s.release_requested = true;
        drop(s);
        self.shared.changed();
        Ok(())
    }
    /// Request cancellation without signalling Release or terminating an owned family.
    pub fn cancel(&self) {
        state(&self.shared).stopped = true;
        self.job.cancel();
    }
}
impl Drop for FixtureSessionControl {
    fn drop(&mut self) {
        self.cancel();
    }
}
/// Pin four execution-owned inheritable duplicates through the launch attribute lifetime.
///
/// This non-cloneable artifact grants no termination or scheduler authority. Drop
/// releases only its pin; the native worker alone checks duplicate closure. Keep
/// it alive through CreateProcess and DeleteProcThreadAttributeList, then drop it.
/// Cancellation cannot complete while it remains owned. Drop before executor join.
#[must_use = "Consume in owned launch input or drop before final execution teardown"]
pub struct FixtureChildEvents {
    pub(crate) shared: Arc<Shared>,
}
impl FixtureChildEvents {
    pub(crate) fn handles(&self) -> [usize; 4] {
        state(&self.shared).handles
    }
}
impl Drop for FixtureChildEvents {
    fn drop(&mut self) {
        state(&self.shared).pin = false;
        self.shared.changed();
    }
}
