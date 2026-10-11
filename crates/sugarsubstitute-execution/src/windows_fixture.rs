// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own four-event fixture protocols through central admission and native retirement.
//! Protocol observations prove event progress only; process and family exits remain
//! separate owners. Cancellation never signals Release or terminates a process.
pub use crate::fixture_forwarding::{
    FixtureForwardingControl, FixtureForwardingEvents, FixtureForwardingPermit,
    FixtureForwardingReport, FixtureForwardingSession,
};
pub use crate::fixture_session::child::{
    FixtureChildAdmissionError, FixtureChildPermit, FixtureChildReport, FixtureDeadline,
    InheritedFixtureEvents,
};
pub use crate::fixture_session::{
    FixtureChildEvents, FixtureObservation, FixtureSession, FixtureSessionControl,
    FixtureSessionError, FixtureSessionReport, MIN_FIXTURE_SESSION_BYTES,
};
use crate::native::NativeObservationError;
use crate::{ExecutionClient, windows::NativeCorrelation};
use std::os::windows::io::BorrowedHandle;
use sugarsubstitute_contracts::execution::{AdmissionError, JobSpec, Scope};

impl FixtureDeadline {
    /// Translate one original parent campaign deadline into fixed absolute QPC ticks.
    ///
    /// QPF/QPC are sampled before reading the remaining `Instant` duration. Checked
    /// integer flooring makes this deadline conservative; pass its unchanged ticks
    /// and frequency to every child. This neither renews the campaign nor changes
    /// the parent's independently admitted execution deadline. Reject expired,
    /// sub-tick, invalid-counter or overflowing input, preserving native clock causes.
    pub fn until(original: std::time::Instant) -> Result<Self, NativeObservationError> {
        deadline::sampled(child_port::sample, || {
            original.checked_duration_since(std::time::Instant::now())
        })
    }
}

impl ExecutionClient {
    /// Reserve forwarding before taking inherited input, with one unchanged QPC deadline.
    /// Reject conflicting local deadlines; cancellation retains a late-bound input until closure.
    pub fn reserve_fixture_forwarding(
        &self,
        scope: &Scope,
        spec: JobSpec,
        correlation: crate::native::NativeCorrelation,
        deadline: FixtureDeadline,
    ) -> Result<FixtureForwardingPermit, FixtureChildAdmissionError> {
        crate::fixture_forwarding::reserve(&self.core, scope, spec, correlation, deadline, || {
            child_port::sample().map_err(FixtureChildAdmissionError::Clock)
        })
    }
    /// Admit one parent session before any native event creation or registration.
    /// Requires a native scope, NativeLifecycle/NativeObservation, a fixed deadline,
    /// and at least MIN_FIXTURE_SESSION_BYTES. All ordinary limits remain unchanged.
    pub fn reserve_fixture_session(
        &self,
        scope: &Scope,
        spec: JobSpec,
        correlation: NativeCorrelation,
    ) -> Result<FixtureSession, AdmissionError> {
        crate::fixture_session::reserve(
            &self.core,
            scope,
            spec,
            correlation,
            Box::new(native::Lease::new()),
        )
    }
}
/// Borrow only the four roles required by the explicit CreateProcess HANDLE_LIST.
/// The launch artifact must outlive attribute deletion. These handles are facts
/// for a trusted native launch adapter, not authentication or termination authority.
pub struct FixtureInheritance<'a> {
    /// Child signals actual readiness after validation and inherit-flag clearing.
    pub ready: BorrowedHandle<'a>,
    /// Child waits for explicit parent Release.
    pub release: BorrowedHandle<'a>,
    /// Child waits for the sole parent Challenge.
    pub challenge: BorrowedHandle<'a>,
    /// Child signals Response only after observing Challenge.
    pub response: BorrowedHandle<'a>,
}
impl FixtureChildEvents {
    /// Borrow four distinct inheritable duplicates while this artifact pins closure.
    /// Keep the returned view and artifact alive through CreateProcess and deleting
    /// its attribute list. Dropping either view performs no native operation.
    #[allow(unsafe_code)]
    pub fn borrow_inheritance(&self) -> FixtureInheritance<'_> {
        let [ready, release, challenge, response] = self.handles();
        // SAFETY: the non-cloneable artifact pins all four execution-owned slots;
        // the native worker cannot close them until its Drop releases that pin.
        unsafe {
            FixtureInheritance {
                ready: BorrowedHandle::borrow_raw(ready as *mut _),
                release: BorrowedHandle::borrow_raw(release as *mut _),
                challenge: BorrowedHandle::borrow_raw(challenge as *mut _),
                response: BorrowedHandle::borrow_raw(response as *mut _),
            }
        }
    }
}
#[allow(unsafe_code)]
#[path = "windows_fixture/child.rs"]
mod child_native;
#[allow(unsafe_code)]
#[path = "windows_fixture/child_port.rs"]
mod child_port;
#[path = "windows_fixture/deadline.rs"]
mod deadline;
#[allow(unsafe_code)]
#[path = "windows_fixture/native.rs"]
mod native;
