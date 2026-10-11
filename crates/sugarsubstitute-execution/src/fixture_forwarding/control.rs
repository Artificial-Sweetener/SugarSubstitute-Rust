// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer an admitted allowlist once and pin native closure through attribute deletion.
use super::*;
use crate::{
    fixture_session::FixtureSessionError,
    task::{ErasedJob, JobHandle},
};
use sugarsubstitute_contracts::execution::{CancelControl, OwnedJob};

/// Reserve before accepting inherited input; consume or drop before final owner join.
#[must_use]
pub struct FixtureForwardingPermit {
    shared: Arc<Shared>,
    job: Arc<Job<FixtureForwardingReport, NativeObservationError>>,
    transferred: bool,
}
impl FixtureForwardingPermit {
    pub(super) fn new(
        shared: Arc<Shared>,
        job: Arc<Job<FixtureForwardingReport, NativeObservationError>>,
    ) -> Self {
        Self { shared, job, transferred: false }
    }
    pub(crate) fn bind_lease(mut self, lease: Box<dyn ForwardLease>) -> FixtureForwardingSession {
        let mut state = lock(&self.shared.state);
        state.lease = Some(lease);
        state.bound = true;
        drop(state);
        self.transferred = true;
        self.shared.changed();
        FixtureForwardingSession {
            control: FixtureForwardingControl {
                shared: self.shared.clone(),
                job: self.job.clone(),
            },
            completion: OwnedJob::new(
                self.job.scope_id,
                self.job.spec,
                Box::new(JobHandle(self.job.clone())),
            ),
        }
    }
}
impl Drop for FixtureForwardingPermit {
    fn drop(&mut self) {
        if !self.transferred {
            let mut state = lock(&self.shared.state);
            state.bound = true;
            state.stopped = true;
            drop(state);
            self.job.abandon();
            self.shared.changed();
        }
    }
}
/// Retain forwarding control and observe terminal only after native closure.
#[must_use]
pub struct FixtureForwardingSession {
    /// Transfer the single prepared allowlist pin.
    pub control: FixtureForwardingControl,
    /// Retain actual closure, cancellation, deadline and original native causes.
    pub completion: OwnedJob<FixtureForwardingReport, NativeObservationError>,
}
/// Queue cancellation or transfer a pin; no method performs a native call.
pub struct FixtureForwardingControl {
    shared: Arc<Shared>,
    job: Arc<Job<FixtureForwardingReport, NativeObservationError>>,
}
impl FixtureForwardingControl {
    /// Transfer once after preparation; a stopped session rejects without native effects.
    pub fn take_events(&self) -> Result<Option<FixtureForwardingEvents>, FixtureSessionError> {
        let mut state = lock(&self.shared.state);
        if state.stopped || self.job.cancel.is_requested() {
            return Err(FixtureSessionError::Closed);
        }
        if state.transferred {
            return Err(FixtureSessionError::AlreadyRequested);
        }
        if !state.prepared {
            return Ok(None);
        }
        state.transferred = true;
        state.pinned = true;
        Ok(Some(FixtureForwardingEvents { shared: self.shared.clone() }))
    }
    /// Retire when the launch pin drops; cancellation never signals or kills a process.
    pub fn cancel(&self) {
        lock(&self.shared.state).stopped = true;
        self.job.cancel();
        self.shared.changed();
    }
}
impl Drop for FixtureForwardingControl {
    fn drop(&mut self) {
        self.cancel();
    }
}
/// Pin four execution-owned duplicates through CreateProcess and attribute deletion.
/// Drop releases only the pin. The native worker checks every original/duplicate close.
#[must_use]
pub struct FixtureForwardingEvents {
    shared: Arc<Shared>,
}
impl FixtureForwardingEvents {
    pub(crate) fn handles(&self) -> [usize; 4] {
        lock(&self.shared.state).handles
    }
}
impl Drop for FixtureForwardingEvents {
    fn drop(&mut self) {
        lock(&self.shared.state).pinned = false;
        self.shared.changed();
    }
}
