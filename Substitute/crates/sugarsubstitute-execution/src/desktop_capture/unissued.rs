// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove the actual pristine worker returned its unused inherited loan without native effects.
use super::{
    control::DesktopCaptureState, metadata::DesktopCaptureCustodyRef, worker::DesktopCaptureActive,
};
use crate::capture_attach::control::CleanupState;
use sugarsubstitute_contracts::execution::{AdmissionError, JobContext};

/// This borrowed marker is constructed only by the actual worker after its concrete
/// inherited owner returned the unused loan. It proves no physical handle closure.
/// No provider, native call, allocation or arbitrary numerical authority is exposed.
pub(crate) struct UnissuedCaptureReturn<'a> {
    active: &'a DesktopCaptureActive,
}
impl<'a> UnissuedCaptureReturn<'a> {
    /// Bind the retained actual worker only after its unused loan was returned.
    pub(super) fn returned(active: &'a DesktopCaptureActive) -> Self {
        Self { active }
    }
    /// Borrow the genuine original admitted context for Core ledger comparison.
    pub(crate) fn context(&self) -> &JobContext {
        &self.active.context
    }
    /// Borrow the actual guarded child allocation without projecting a raw Arc.
    pub(crate) fn custody(&self) -> &DesktopCaptureCustodyRef {
        &self.active.work.shared
    }
    /// Observe actual worker vacancy under Core's existing cleanup/stage fence.
    /// Returning an unused loan is distinct from physical native/source retirement.
    pub(crate) fn validate_locked(
        &self,
        cleanup: &CleanupState,
        stage: &DesktopCaptureState,
    ) -> Result<(), AdmissionError> {
        let active = self.active;
        if !active.input_returned
            || active.work.inherited.is_some()
            || active.work.driver.is_none()
            || active.event.is_some()
            || active.proof.is_some()
            || active.prepared
            || active.attached
            || active.completed
            || active.work.capsule.as_ref().is_none_or(|capsule| capsule.storage.fields.is_some())
            || !cleanup.stopped
            || !cleanup.active
            || stage.registered_published
            || stage.phase != super::control::DesktopCapturePhase::Cleaning
            || stage.accepted_stage.is_some()
            || stage.accepted_cleanup.is_some()
            || stage.last_nonce != 0
            || stage.last_cleanup != 0
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        Ok(())
    }
}
