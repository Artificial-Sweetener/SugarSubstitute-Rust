// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain parent controller metadata from the actual original custody pipe allocation.
use super::{BackendCustodyPipeControl, VerifiedBackendCustodyPeer};
use crate::{
    lifecycle_pipe::{Owner, PeerBinding, SharedRef},
    scope::lock,
};
use sugarsubstitute_contracts::{
    backend_custody::BackendCustodyKey,
    execution::{AdmissionError, JobPhase, NativeResourceError},
};
/// Fixed parent metadata alias; the native pipe's original64KiB claim stays outside
/// every controller allocation. This grants no raw handles, context or claim access.
#[derive(Clone)]
pub struct BackendCustodyParentStorage {
    source: SharedRef,
    owner: Owner,
    key: BackendCustodyKey,
    _metadata: crate::lifecycle_pipe::metadata::Metadata,
}
impl BackendCustodyParentStorage {
    /// Match the identical once-issued parent metadata allocation, without caller identities.
    pub fn same_allocation(&self, other: &Self) -> bool {
        self.source.same_allocation(&other.source)
    }
    /// Compare the original actual transport Core with a receiving execution endpoint.
    /// This is a provider-free association check; callers separately check current admission.
    pub fn matches_execution(&self, execution: &crate::ExecutionClient) -> bool {
        self.source.core.ptr_eq(&std::sync::Arc::downgrade(&execution.core))
    }
    /// Revalidate the actual parent transport with the caller's same Core guard.
    pub(crate) fn validate_locked(
        &self,
        core: &std::sync::Arc<crate::admission::Core>,
        state: &crate::admission::State,
        now: sugarsubstitute_contracts::execution::Moment,
    ) -> Result<(), AdmissionError> {
        if self.source.child || !self.source.core.ptr_eq(&std::sync::Arc::downgrade(core)) {
            return Err(AdmissionError::NativeResources(
                NativeResourceError::OriginalAdmissionRetired,
            ));
        }
        crate::lifecycle_pipe::current_admission::source_locked(
            core,
            state,
            &self.source,
            &self.owner,
            now,
        )
    }
    /// Observe the exact original session key as matching facts only.
    pub const fn key(&self) -> BackendCustodyKey {
        self.key
    }
    /// Return the unchanged original metadata partition, in bytes.
    pub const fn bytes(&self) -> u64 {
        65_536
    }
    /// Count the native producer's complete known storage before a controller allocation.
    #[cfg(windows)]
    pub const fn producer_storage_bytes() -> usize {
        crate::windows_lifecycle_pipe::native::Lease::storage_bytes()
    }
    /// Match only the actual original control allocation, without providers.
    pub fn matches_session(&self, control: &BackendCustodyPipeControl) -> bool {
        self.source.same_allocation(&control.inner.shared)
    }
    /// Revalidate the actual authenticated helper/current original source before effects.
    /// # Errors
    /// Preserves original provider, scope, cancellation or stopped-source causes.
    pub fn check(&self) -> Result<(), crate::native::NativeObservationError> {
        crate::lifecycle_pipe::validate_current_source(&self.source, &self.owner)
    }
    /// Borrow the actual cached authenticated helper identity after current admission.
    /// No PID or matching key can construct this sealed transport observation.
    /// # Errors
    /// Rejects unauthenticated or mismatched original peers.
    pub fn peer(
        &self,
    ) -> Result<VerifiedBackendCustodyPeer, crate::native::NativeObservationError> {
        self.check()?;
        let observed = lock(&self.source.state).authenticated_peer;
        let peer = observed
            .filter(|peer| {
                peer.binding.backend_custody() == Some(self.key) && peer.process.is_some()
            })
            .ok_or_else(|| {
                crate::native::NativeObservationError::new(
                    "custody_parent_peer",
                    crate::lifecycle_pipe::LifecyclePipeError::NotReady,
                )
            })?;
        Ok(VerifiedBackendCustodyPeer {
            key: self.key,
            process: peer.process,
            pid: peer.pid,
            birth: peer.birth,
            session: peer.session,
        })
    }
}
impl BackendCustodyPipeControl {
    /// Reserve the one controller allocation from this actual admitted server's metadata.
    /// Queued/running original work permits inert allocation. Fresh native effects still
    /// require kernel/capability authentication and `BackendCustodyParentStorage::check`.
    /// # Errors
    /// Rejects child/foreign/repeated/closed/expired allocations without new capacity.
    pub fn take_parent_storage(&self) -> Result<BackendCustodyParentStorage, AdmissionError> {
        let core = self.inner.shared.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
        let source = self.inner.shared.clone();
        let owner = self.inner.owner.clone();
        let installed = source.metadata.installed();
        let now = core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let checked = (|| {
            let state = lock(&core.state);
            let record = state.jobs.get(&source.id).ok_or(AdmissionError::ShuttingDown)?;
            if source.child
                || !owner.matches_record(record)
                || record.retiring
                || !matches!(record.phase, JobPhase::Queued | JobPhase::Running)
                || !installed
            {
                return Err(AdmissionError::NativeResources(
                    NativeResourceError::OriginalAdmissionRetired,
                ));
            }
            let cutoff = crate::admission_liveness::scope_deadline(&core, &state, record)?;
            if let Some(deadline) = cutoff.filter(|until| *until <= now) {
                return Err(AdmissionError::DeadlineExpired { deadline, now });
            }
            let mut pipe = lock(&source.state);
            let key = pipe.binding.and_then(PeerBinding::backend_custody).ok_or(
                AdmissionError::NativeResources(NativeResourceError::OriginalAdmissionRetired),
            )?;
            if pipe.stopped || pipe.custody_parent_storage_taken || source.original_deadline <= now
            {
                return Err(AdmissionError::NativeResources(
                    NativeResourceError::OriginalAdmissionRetired,
                ));
            }
            pipe.custody_parent_storage_taken = true;
            Ok(key)
        })();
        let key = checked?;
        Ok(BackendCustodyParentStorage { _metadata: source.metadata.clone(), source, owner, key })
    }
}
