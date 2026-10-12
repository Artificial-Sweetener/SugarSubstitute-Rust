// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own capture preparation, stop claims and native custody independently of results.
pub(crate) mod admission;
#[cfg(test)]
mod cleanup_tests;
#[cfg(test)]
mod conflict_race_tests;
#[cfg(test)]
mod conflict_tests;
pub(crate) mod control;
#[cfg(test)]
mod custody_tests;
pub(crate) mod issue;
#[cfg(test)]
mod issue_tests;
#[cfg(test)]
mod scope_sampling_tests;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
pub(crate) mod types;
pub(crate) mod worker;

use crate::{
    admission::Core,
    native::{NativeObservationError, Signal},
    scope::lock,
    task::Job,
};
use control::{CaptureAttachRetirementReport, CaptureCleanupSnapshot, Custody};
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::execution::{JobContext, Moment};
use types::{CaptureAttachFailure, CaptureEndpointObservation, CaptureEndpointSource};

/// A private actual native lease, retained outside every fallible operation.
pub(crate) trait AttachLease: Send {
    fn prepare(
        &mut self,
        observed: CaptureEndpointObservation,
        context: &JobContext,
        signal: Arc<Signal>,
        shared: &Arc<Shared>,
    ) -> Result<(), NativeObservationError>;
    fn progress(&mut self) -> Result<Option<[usize; 2]>, NativeObservationError>;
    /// Prove temporary callbacks/I/O are retired before publishing process pins.
    fn prepared(&self) -> bool;
    /// Close only when no launch/session borrow remains; never terminate either process.
    fn retire(
        &mut self,
        borrowed: bool,
        cleanup: &CaptureCleanupSnapshot,
    ) -> Result<bool, NativeObservationError>;
}
pub(crate) struct PinState {
    pub(crate) pinned: bool,
    pub(crate) handles: Option<[usize; 2]>,
    pub(crate) endpoint: Option<CaptureEndpointObservation>,
    pub(crate) retirement: Option<Result<CaptureAttachRetirementReport, CaptureAttachFailure>>,
    pub(crate) retirement_published: bool,
}
pub(crate) struct Shared {
    pub(crate) custody: Arc<Custody>,
    pub(crate) state: Mutex<PinState>,
}
impl Shared {
    fn release(&self) {
        lock(&self.state).pinned = false;
        control::request(&self.custody, control::CleanupTrigger::Released);
        if let Some(core) = self.custody.core.upgrade() {
            core.changed();
        }
    }
    pub(crate) fn publish_retirement(
        &self,
        core: &Core,
        report: Result<CaptureAttachRetirementReport, CaptureAttachFailure>,
    ) {
        let mut state = lock(&self.state);
        if state.retirement_published {
            return;
        }
        state.retirement_published = true;
        state.retirement = Some(report);
        drop(state);
        if core.mailbox.signal_capacity() {
            core.mailbox.deliver_wake();
        }
    }
}

/// Retain verified process/source custody for exactly one captured desktop launch.
/// Construction is private to successful execution-owned native preparation.
/// Drop requests the original cleanup transaction and performs no native operation.
#[must_use = "Transfer the capture pin into the captured lifecycle session or release it"]
pub struct CaptureAttachLaunchPin {
    pub(crate) shared: Arc<Shared>,
    pub(crate) released: bool,
}
impl CaptureAttachLaunchPin {
    /// Release the exclusive borrow and observe actual retirement separately.
    /// The single first cleanup deadline is latched by the original execution owner.
    pub fn release(mut self) -> CaptureAttachRetirement {
        self.shared.release();
        self.released = true;
        CaptureAttachRetirement { shared: self.shared.clone() }
    }
}
impl Drop for CaptureAttachLaunchPin {
    fn drop(&mut self) {
        if !self.released {
            self.shared.release();
        }
    }
}

/// Observe one bounded retirement result without borrowing process handles.
/// Dropping this observer abandons delivery; the original worker retains custody.
pub struct CaptureAttachRetirement {
    shared: Arc<Shared>,
}
impl CaptureAttachRetirement {
    /// Consume the original checked-retirement report or original unresolved failure once.
    /// A late actual native close never rewrites an already published failure.
    pub fn try_take_report(
        &mut self,
    ) -> Option<Result<CaptureAttachRetirementReport, CaptureAttachFailure>> {
        lock(&self.shared.state).retirement.take()
    }
}

pub(crate) struct AttachWork {
    pub(crate) shared: Arc<Shared>,
    pub(crate) source: Arc<dyn CaptureEndpointSource>,
    pub(crate) job: Arc<Job<CaptureAttachLaunchPin, CaptureAttachFailure>>,
    pub(crate) lease: Box<dyn AttachLease>,
    pub(crate) handshake: Moment,
}
