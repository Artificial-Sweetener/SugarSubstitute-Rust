// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Translate the original absolute deadline before admitting inherited ownership.
use super::*;
use crate::fixture_session::child::{
    self, FixtureChildAdmissionError, FixtureChildPermit, FixtureChildReport, FixtureDeadline,
    InheritedFixtureEvents,
};
use crate::native::NativeObservationError;
use sugarsubstitute_contracts::execution::OwnedJob;

impl ExecutionClient {
    /// Reserve child lifetime before passing inherited input; takes no handle ownership.
    ///
    /// `spec.deadline` must be None. The port samples its local Moment BEFORE QPC,
    /// requires the exact enrolled frequency, and floors remaining nanoseconds.
    /// It sets that conservative fixed deadline before central admission. Clock,
    /// expiry or admission failure leaves all inherited input with the isolated
    /// caller, which must fail/exit for the parent to observe actual OS settlement.
    /// QPC/QPF are finite clock queries; no wait or event operation occurs here.
    pub fn reserve_fixture_child(
        &self,
        scope: &Scope,
        spec: JobSpec,
        correlation: NativeCorrelation,
        deadline: FixtureDeadline,
    ) -> Result<FixtureChildPermit, FixtureChildAdmissionError> {
        child::reserve_sampled(&self.core, scope, spec, correlation, deadline, || {
            crate::windows_counter::sample().map_err(FixtureChildAdmissionError::Clock)
        })
    }
}
impl FixtureChildPermit {
    /// Transfer exclusive inherited ownership without another rejection/admission.
    /// The native worker clears inheritance, validates roles, registers/signals and
    /// closes every transferred handle. Cancellation before bind still retires
    /// input before terminal; it never kills or automatically releases a process.
    pub fn bind(
        self,
        events: InheritedFixtureEvents,
    ) -> OwnedJob<FixtureChildReport, NativeObservationError> {
        self.bind_lease(Box::new(super::child_native::Lease::new(events)))
    }
}
