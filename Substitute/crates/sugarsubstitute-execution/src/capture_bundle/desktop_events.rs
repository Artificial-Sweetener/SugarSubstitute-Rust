// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Issue exactly the original child's Event and Wait credits from its actual record.

use super::CaptureBundleOrigin;
use crate::{
    admission::{Core, Record, State},
    desktop_capture::{
        metadata::DesktopCaptureCustodyRef,
        resources::{CaptureEventClaim, EVENT_METADATA_BYTES},
    },
    scope::lock,
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::AdmissionError;

/// Match only sealed origin, actual retained memory and this child's native custody.
fn association<'a>(
    core: &Core,
    state: &'a State,
    origin: &CaptureBundleOrigin,
    custody: &DesktopCaptureCustodyRef,
) -> Result<&'a Record, AdmissionError> {
    let original = origin.state()?;
    if !original.desktop_child || !std::ptr::eq(original.core.as_ptr(), core) {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    let record = super::validate::association(core, state, &original.original)?;
    if record.desktop_capture_custody.as_ref().is_none_or(|actual| !actual.same(custody))
        || record
            .native_custody
            .as_ref()
            .is_none_or(|actual| !Arc::ptr_eq(actual, &custody.cleanup))
        || custody.cleanup.execution != core.id
        || custody.cleanup.job != record.spec.id
    {
        return Err(AdmissionError::InvalidCaptureAdmission);
    }
    Ok(record)
}

impl Core {
    /// Preflight and consume the sole4096 subdivision before allocating its controls.
    /// This remains part of the original966656 native/input claim. No Symbol lease,
    /// new job, reservation or arbitrary output count is created.
    /// Every provider and claim/control destruction occurs outside owner fences.
    pub(crate) fn desktop_capture_event_claim(
        self: &Arc<Self>,
        origin: &CaptureBundleOrigin,
        custody: &DesktopCaptureCustodyRef,
    ) -> Result<CaptureEventClaim, AdmissionError> {
        if CaptureEventClaim::storage_bytes() > EVENT_METADATA_BYTES as usize {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        {
            let state = lock(&self.state);
            let record = association(self, &state, origin, custody)?;
            if record.capture_event_issued {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
        }
        let now = self.clock.metadata().map_err(AdmissionError::Unexpected)?;
        {
            let mut state = lock(&self.state);
            let original = origin.state()?;
            super::validate::effect(self, &state, &original.original, now, true)?;
            let record = association(self, &state, origin, custody)?;
            if record.capture_event_issued {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            let job = record.spec.id;
            state
                .jobs
                .get_mut(&job)
                .ok_or(AdmissionError::InvalidCaptureAdmission)?
                .capture_event_issued = true;
        }
        let memory = origin
            .original_context()?
            .memory()
            .claim(EVENT_METADATA_BYTES)
            .map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?;
        let claim = CaptureEventClaim::new(origin.clone(), Arc::downgrade(self), memory);
        // A cancellation or cleanup callback during construction must veto first output.
        let now = self.clock.metadata().map_err(AdmissionError::Unexpected)?;
        {
            let mut state = lock(&self.state);
            super::validate::effect(self, &state, origin.original_context()?, now, true)?;
            let record = association(self, &state, origin, custody)?;
            if !record.capture_event_issued || record.capture_event_identity.is_some() {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            let job = record.spec.id;
            state
                .jobs
                .get_mut(&job)
                .ok_or(AdmissionError::InvalidCaptureAdmission)?
                .capture_event_identity = Some(claim.identity());
        }
        Ok(claim)
    }

    /// Validate this exact registered counter allocation before each fresh output effect.
    /// A matching original ledger alone cannot substitute a different counter allocation.
    pub(crate) fn check_capture_event(
        &self,
        origin: &CaptureBundleOrigin,
        identity: usize,
    ) -> Result<(), AdmissionError> {
        let original = origin.state()?;
        if !original.desktop_child || !std::ptr::eq(original.core.as_ptr(), self) {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        let now = self.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = lock(&self.state);
        super::validate::effect(self, &state, &original.original, now, true)?;
        let record = super::validate::association(self, &state, &original.original)?;
        let custody = record
            .desktop_capture_custody
            .as_ref()
            .ok_or(AdmissionError::InvalidCaptureAdmission)?;
        association(self, &state, origin, custody)?;
        if !record.capture_event_issued || record.capture_event_identity != Some(identity) {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        Ok(())
    }
}
