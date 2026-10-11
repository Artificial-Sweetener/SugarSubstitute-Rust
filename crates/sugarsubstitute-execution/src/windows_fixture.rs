// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own four-event fixture protocols through central admission and native retirement.
//! Protocol observations prove event progress only; process and family exits remain
//! separate owners. Cancellation never signals Release or terminates a process.
pub use crate::fixture_session::child::{
    FixtureChildAdmissionError, FixtureChildPermit, FixtureChildReport, FixtureDeadline,
    InheritedFixtureEvents,
};
pub use crate::fixture_session::{
    FixtureChildEvents, FixtureObservation, FixtureSession, FixtureSessionControl,
    FixtureSessionError, FixtureSessionReport, MIN_FIXTURE_SESSION_BYTES,
};
use crate::{ExecutionClient, windows::NativeCorrelation};
use std::os::windows::io::BorrowedHandle;
use sugarsubstitute_contracts::execution::{AdmissionError, JobSpec, Scope};

impl ExecutionClient {
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
#[allow(unsafe_code)]
#[path = "windows_fixture/native.rs"]
mod native;
