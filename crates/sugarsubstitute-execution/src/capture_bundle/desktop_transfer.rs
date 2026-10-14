// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve genuine permanent capture association after temporary worker settlement.

use crate::{
    admission::Core,
    desktop_capture::{
        control::DesktopCapturePhase,
        event::CompletionEvent,
        process_lifetime::{ProcessCaptureCapsule, TemporaryCaptureSettlement},
    },
    scope::lock,
};
use std::sync::Arc;
use sugarsubstitute_contracts::{crash::CaptureTarget, execution::AdmissionError};

/// Only a successful actual capsule transfer installs these inert conflict facts.
/// No origin, context, claim, Arc/Weak capsule, native pointer or release port is retained.
/// The original outside capsule continues to own all four MiB and permanent source.
pub(crate) struct ProcessCaptureAssociation {
    pub(crate) full_target: CaptureTarget,
    pub(crate) capsule_identity: usize,
}

impl Core {
    /// Transfer already-proven temporary custody into the same permanent outside capsule.
    /// Both inputs remain owned by the caller on every rejection. The final fence uses
    /// only correlated adopted facts; it samples no provider, issues no native work and
    /// creates no new Ready result or deadline. Intentional cancellation after genuine
    /// Registered publication does not prevent adoption of its accepted close returns.
    pub(crate) fn transfer_desktop_capture(
        &self,
        capsule: &ProcessCaptureCapsule,
        event: &CompletionEvent,
    ) -> Result<(), AdmissionError> {
        let origin = capsule.original()?;
        let original = origin.state()?;
        let custody = capsule.custody()?;
        if !original.desktop_child || !std::ptr::eq(original.core.as_ptr(), self) {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        let detached = {
            let mut state = lock(&self.state);
            let record = super::validate::association(self, &state, &original.original)?;
            if !record.capture_bundle_issued
                || !record.capture_event_issued
                || record.process_capture.is_some()
                || !record.terminal_published
                || record
                    .desktop_capture_custody
                    .as_ref()
                    .is_none_or(|actual| !actual.same(custody))
                || record
                    .native_custody
                    .as_ref()
                    .is_none_or(|actual| !Arc::ptr_eq(actual, &custody.cleanup))
                || custody.cleanup.execution != self.id
                || custody.cleanup.job != record.spec.id
            {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            let event_identity =
                record.capture_event_identity.ok_or(AdmissionError::InvalidCaptureAdmission)?;
            let job_id = record.spec.id;
            let mut cleanup = lock(&custody.cleanup.state);
            let mut stage = lock(&custody.state);
            let settled = TemporaryCaptureSettlement::validate_locked(
                capsule,
                event,
                event_identity,
                &cleanup,
                &stage,
            )?;
            if !settled.origin().same_allocation(origin) || !settled.custody().same(custody) {
                return Err(AdmissionError::InvalidCaptureAdmission);
            }
            let association = ProcessCaptureAssociation {
                full_target: custody.cleanup.full_target,
                capsule_identity: settled.capsule_identity(),
            };
            let record =
                state.jobs.get_mut(&job_id).ok_or(AdmissionError::InvalidCaptureAdmission)?;
            record.process_capture = Some(association);
            record.native_deadlines = None;
            stage.phase = DesktopCapturePhase::Transferred;
            cleanup.active = false;
            // Native/source retirement flags remain physical facts. The permanent
            // process capsule still owns them; no guard or admission byte is reclaimed.
            let detached = (record.native_custody.take(), record.desktop_capture_custody.take());
            drop(stage);
            drop(cleanup);
            drop(state);
            detached
        };
        // Metadata and any final admitted lease must be destroyed after all fences.
        drop(detached);
        self.retire_native_custody(original.original.job_id());
        self.changed();
        Ok(())
    }
}
