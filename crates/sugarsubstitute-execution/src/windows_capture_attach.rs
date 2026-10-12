// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticate capture attachment through retained kernel owners and finite native I/O.
//!
//! Public facts contain no native handles. The execution owner constructs launch
//! custody only after validating the real registered source and actual kernel peer.
pub use crate::capture_attach::admission::{
    CaptureAttachAdmissionError, CaptureAttachAdmissionRejection,
};
pub use crate::capture_attach::{CaptureAttachLaunchPin, CaptureAttachRetirement};
pub use crate::capture_attach::{
    control::{
        CaptureAttachRetirementReport, CaptureCleanupControl, CaptureCleanupCounterBudget,
        CaptureCleanupSnapshot,
    },
    types::{
        CaptureAttachDataError, CaptureAttachExpectation, CaptureAttachFailure,
        CaptureEndpointObservation, CaptureEndpointProvenance, CaptureEndpointSource,
        CaptureEndpointSourceError, CaptureProcessFacts,
    },
};

mod native;
mod ping;
#[cfg(test)]
mod tests;
use crate::{ExecutionClient, capture_attach};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobSpec, OwnedJob, Scope};
impl ExecutionClient {
    /// Reserve original capture attachment before acquiring any native process or endpoint.
    /// The persistent native scope and every ancestor must have no deadline.
    /// Preparation uses the exact sixty-second expectation; cleanup starts only on
    /// the first actual failure/cancel/release/shutdown. Cancellation/result drop
    /// retains original native/source/memory custody until checked retirement.
    /// # Errors
    /// Returns the exact untransferred expectation and source on admission rejection.
    pub fn reserve_capture_attach_pin(
        &self,
        scope: &Scope,
        spec: JobSpec,
        expected: CaptureAttachExpectation,
        source: Arc<dyn CaptureEndpointSource>,
    ) -> Result<
        OwnedJob<CaptureAttachLaunchPin, CaptureAttachFailure>,
        Box<CaptureAttachAdmissionRejection>,
    > {
        capture_attach::admission::reserve(
            &self.core,
            scope,
            spec,
            expected,
            source,
            Box::new(native::Lease::new(expected)),
        )
    }
}
