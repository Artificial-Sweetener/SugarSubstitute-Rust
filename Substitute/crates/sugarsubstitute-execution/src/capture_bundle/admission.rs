// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Issue one bundle claim from the actual fixed original running capture job.

use super::{
    CaptureBundleAdmission, CaptureBundleOrigin,
    origin::{BUNDLE_BYTES, CleanupLatch, OriginState},
};
use crate::{ExecutionClient, scope::lock};
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::execution::{AdmissionError, JobContext};

impl ExecutionClient {
    /// Consume the original record's sole two-MiB bundle issuance before native effects.
    ///
    /// Supplied context ports are never invoked. The producer validates the actual
    /// ledger, then reconstructs Core clock and cancellation ports from its record.
    /// New origin control storage is preflighted against the fixed metadata region.
    ///
    /// # Errors
    /// Rejects foreign, changed, nonrunning, cancelled, expired, wrongly reserved or
    /// already-issued records. Claim failure preserves its actual accounting cause;
    /// clock failure preserves the original cause and the owner's actual shutdown.
    pub fn capture_bundle_admission(
        &self,
        observed: &JobContext,
    ) -> Result<CaptureBundleAdmission, AdmissionError> {
        if !CaptureBundleOrigin::layout_fits() {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        {
            let state = lock(&self.core.state);
            super::validate::association(&self.core, &state, observed)?;
        }
        let now = self.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let (original, memory, desktop_child) = {
            let mut state = lock(&self.core.state);
            super::validate::effect(&self.core, &state, observed, now, false)?;
            let record = state
                .jobs
                .get_mut(&observed.job_id())
                .ok_or(AdmissionError::InvalidCaptureAdmission)?;
            let memory = observed
                .memory()
                .claim(BUNDLE_BYTES)
                .map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?;
            let original = super::validate::reconstruct(&self.core, record, observed);
            record.capture_bundle_issued = true;
            (original, memory, record.desktop_capture_child())
        };
        // Both owning guards leave Core before allocation or any final destruction.
        Ok(CaptureBundleAdmission {
            origin: CaptureBundleOrigin(Some(Arc::new(OriginState {
                core: Arc::downgrade(&self.core),
                original,
                desktop_child,
                cleanup: Mutex::new(CleanupLatch::Dormant),
                _memory: memory,
            }))),
        })
    }
}
