// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind custody-worker issuance to one actual authenticated child session allocation.
use super::BackendCustodyPipeControl;
use crate::lifecycle_pipe::{self, Owner, PeerBinding, SharedRef};
use sugarsubstitute_contracts::backend_custody::BackendCustodyKey;

/// Authorize one custody worker from the actual live child transport allocation.
/// Numeric key and historical peer facts cannot construct this nonclone value.
/// Rejected worker admission must return the complete original permit.
pub struct BackendCustodyWorkerPermit {
    pub(crate) shared: SharedRef,
    pub(crate) owner: Owner,
    key: BackendCustodyKey,
}
impl BackendCustodyWorkerPermit {
    /// Return immutable matching facts without exposing native handles or a scheduler.
    pub const fn key(&self) -> BackendCustodyKey {
        self.key
    }
    /// Revalidate the original live authenticated transport before worker admission.
    /// # Errors
    /// Retains actual clock/admission failure or closed transport cause.
    pub fn validate_current_admission(&self) -> Result<(), crate::native::NativeObservationError> {
        lifecycle_pipe::validate_current_source(&self.shared, &self.owner)
    }
}
impl BackendCustodyPipeControl {
    /// Issue exactly one nonclone worker permit from the actual authenticated child.
    /// The final Core->Shared fence rechecks scope, job and current authentication
    /// before setting the sticky issuance bit; it invokes no external callback.
    /// # Errors
    /// Rejects parent projections, foreign purposes, repeated issuance or stopped work.
    pub fn take_worker_permit(
        &self,
    ) -> Result<BackendCustodyWorkerPermit, crate::native::NativeObservationError> {
        lifecycle_pipe::with_current_session(&self.inner, |state| {
            if !self.inner.shared.child {
                return Err(lifecycle_pipe::LifecyclePipeError::NotReady);
            }
            let key = state
                .binding
                .and_then(PeerBinding::backend_custody)
                .ok_or(lifecycle_pipe::LifecyclePipeError::NotReady)?;
            if state.custody_worker_permit_taken {
                return Err(lifecycle_pipe::LifecyclePipeError::AlreadyTransferred);
            }
            state.custody_worker_permit_taken = true;
            Ok(BackendCustodyWorkerPermit {
                shared: self.inner.shared.clone(),
                owner: self.inner.owner.clone(),
                key,
            })
        })
    }
}
