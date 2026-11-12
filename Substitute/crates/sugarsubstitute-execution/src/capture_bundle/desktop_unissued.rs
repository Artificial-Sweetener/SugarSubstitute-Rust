// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Settle only a genuine pristine child whose unused inherited loan was returned.

use crate::{
    admission::Core,
    desktop_capture::{control::DesktopCapturePhase, unissued::UnissuedCaptureReturn},
    scope::lock,
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{AdmissionError, JobClass, Operation};

impl Core {
    /// Detach the actual unissued child's controls after its unused loan returns.
    /// The marker borrows its same worker, original context and vacant capsule.
    /// No clock, native call, callback, allocation or fresh effect permission occurs
    /// here. Original cancellation, expiry and shutdown do not prevent returning
    /// that existing loan. Physical map/process/source retirement remains with
    /// its retained lifecycle owner; no physical retirement flag is changed.
    pub(crate) fn retire_unissued_desktop_capture(
        &self,
        returned: &UnissuedCaptureReturn<'_>,
    ) -> Result<(), AdmissionError> {
        let context = returned.context();
        let custody = returned.custody();
        let detached = {
            let mut state = lock(&self.state);
            let record = super::validate::association(self, &state, context)?;
            if record.spec.operation != Operation::NativeLifecycle
                || record.spec.class != JobClass::InfrastructureLifecycle
                || record.spec.reservation_bytes != super::origin::RESERVATION_BYTES
                || record.spec.replacement.is_some()
                || record.deadline.is_none()
                || record.capture_bundle_issued
                || record.capture_event_issued
                || record.capture_event_identity.is_some()
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
            let id = record.spec.id;
            let mut cleanup = lock(&custody.cleanup.state);
            let mut stage = lock(&custody.state);
            returned.validate_locked(&cleanup, &stage)?;
            let record = state.jobs.get_mut(&id).ok_or(AdmissionError::InvalidCaptureAdmission)?;
            record.native_deadlines = None;
            stage.phase = DesktopCapturePhase::UnissuedReturned;
            cleanup.active = false;
            let detached = (record.native_custody.take(), record.desktop_capture_custody.take());
            drop(stage);
            drop(cleanup);
            drop(state);
            detached
        };
        // The final control/claim destructors can re-enter Core only after unlock.
        drop(detached);
        self.retire_native_custody(context.job_id());
        self.changed();
        Ok(())
    }
}
