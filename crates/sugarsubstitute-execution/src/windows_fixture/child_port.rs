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
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

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
        child::reserve_sampled(&self.core, scope, spec, correlation, deadline, sample)
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
fn sample() -> Result<(u64, u64), FixtureChildAdmissionError> {
    let mut frequency = 0;
    let mut ticks = 0;
    // SAFETY: bounded native clock queries write only these live signed counters.
    unsafe { QueryPerformanceFrequency(&mut frequency) }.map_err(|cause| {
        FixtureChildAdmissionError::Clock(NativeObservationError::new(
            "fixture_qpc_frequency",
            cause,
        ))
    })?;
    // SAFETY: finite clock query writes the live signed counter only.
    unsafe { QueryPerformanceCounter(&mut ticks) }.map_err(|cause| {
        FixtureChildAdmissionError::Clock(NativeObservationError::new("fixture_qpc_counter", cause))
    })?;
    if frequency <= 0 || ticks <= 0 {
        return Err(FixtureChildAdmissionError::Clock(NativeObservationError::new(
            "fixture_positive_clock",
            std::io::Error::other("QPC/QPF returned nonpositive values"),
        )));
    }
    Ok((ticks as u64, frequency as u64))
}
