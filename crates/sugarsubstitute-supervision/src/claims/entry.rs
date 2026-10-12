// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind entry publication and serving authority to actual retained claims lifetimes.
use super::*;
use crate::{
    WindowsProcessRegistry,
    coordinator_entry::election::{
        ElectionInitializationGuard, ElectionRecordOrigin, FreshRecordOrigin, RetainedElectionProof,
    },
    protected_storage::{exclusion::HeldFile, record_file::RecordRead},
};
use sugarsubstitute_contracts::execution::ExecutionId;

#[derive(Clone)]
pub(crate) struct ClaimAdmissionLifetime {
    pub(crate) origin: Arc<()>,
    pub(crate) admission: Arc<AtomicBool>,
}

pub(crate) struct RetainedCoordinatorProof {
    owner: CoordinatorKey,
    execution: ExecutionId,
    origin: Arc<()>,
    admission: Arc<AtomicBool>,
    initialized_held: Arc<AtomicBool>,
    registry: RegistryIdentity,
}
impl RetainedCoordinatorProof {
    pub(crate) fn validate_live(
        &self,
        owner: CoordinatorKey,
        execution: ExecutionId,
    ) -> Result<(), ClaimAdmissionError> {
        if self.owner != owner || self.execution != execution {
            return Err(ClaimAdmissionError::Identity);
        }
        if !self.admission.load(Ordering::Acquire) || !self.initialized_held.load(Ordering::Acquire)
        {
            return Err(ClaimAdmissionError::Closed);
        }
        Ok(())
    }
    pub(crate) fn validate_registry(
        &self,
        registry: &WindowsProcessRegistry,
    ) -> Result<(), ClaimAdmissionError> {
        let identity =
            registry.claim_identity(self.owner).map_err(ClaimAdmissionError::Registry)?;
        if self.registry.same_as(&identity) { Ok(()) } else { Err(ClaimAdmissionError::Identity) }
    }
    pub(crate) fn belongs_to(&self, claims: &WindowsResourceClaims) -> bool {
        Arc::ptr_eq(&self.origin, &claims.origin)
    }
}
impl WindowsResourceClaims {
    pub(crate) fn bind_entry_initialization(
        &mut self,
        mut proof: RetainedElectionProof,
    ) -> Result<(), ClaimAdmissionError> {
        if self.initialization_started
            || self.initialization_pending
            || self.closed
            || !self.open.load(Ordering::Acquire)
        {
            return Err(ClaimAdmissionError::Closed);
        }
        if self.entry_initialization.is_some()
            || self.execution_owner.is_some_and(|owner| owner != proof.execution)
        {
            return Err(ClaimAdmissionError::Identity);
        }
        proof.validate_live().map_err(ClaimAdmissionError::Native)?;
        proof.claim_lifetime = Some(ClaimAdmissionLifetime {
            origin: self.origin.clone(),
            admission: self.open.clone(),
        });
        self.entry_initialization = Some(proof);
        Ok(())
    }
    pub(crate) fn retained_coordinator(
        &self,
        registry: &WindowsProcessRegistry,
        owner: CoordinatorKey,
        execution: ExecutionId,
    ) -> Result<RetainedCoordinatorProof, ClaimAdmissionError> {
        if self.owner != Some(owner) || self.execution_owner != Some(execution) {
            return Err(ClaimAdmissionError::Identity);
        }
        if !self.open.load(Ordering::Acquire)
            || !self.entry_initialized_held.load(Ordering::Acquire)
            || !self.initialized
            || self.initialization_pending
            || self.closed
        {
            return Err(ClaimAdmissionError::Closed);
        }
        let identity = registry.claim_identity(owner).map_err(ClaimAdmissionError::Registry)?;
        if !self.registry.as_ref().is_some_and(|bound| bound.same_as(&identity)) {
            return Err(ClaimAdmissionError::Identity);
        }
        Ok(RetainedCoordinatorProof {
            owner,
            execution,
            origin: self.origin.clone(),
            admission: self.open.clone(),
            initialized_held: self.entry_initialized_held.clone(),
            registry: identity,
        })
    }
}

pub(crate) struct FreshCoordinatorPublication<'a> {
    guard: &'a ElectionInitializationGuard<'a>,
    coordinator: &'a HeldFile<128>,
    coordinator_inode: [u8; 24],
    claim_origin: Arc<()>,
    claim_admission: Arc<AtomicBool>,
}
impl<'a> ElectionInitializationGuard<'a> {
    pub(crate) fn publication<'b>(
        &'b self,
        file: &'b HeldFile<128>,
        coordinator_inode: [u8; 24],
    ) -> Result<FreshCoordinatorPublication<'b>, ClaimError> {
        if !matches!(self.proof.origin, ElectionRecordOrigin::Fresh(_)) {
            return Err(ClaimError::invalid("released_origin_is_ordinary"));
        }
        self.check_coordinator(file, coordinator_inode)?;
        let claim = self
            .proof
            .claim_lifetime
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("entry_claim_lifetime_missing"))?;
        let claim_origin = claim.origin.clone();
        let claim_admission = claim.admission.clone();
        if !claim_admission.load(Ordering::Acquire) {
            return Err(ClaimError::invalid("entry_claim_admission_closed"));
        }
        Ok(FreshCoordinatorPublication {
            guard: self,
            coordinator: file,
            coordinator_inode,
            claim_origin,
            claim_admission,
        })
    }
}
impl FreshCoordinatorPublication<'_> {
    pub(crate) fn validate<const N: usize>(
        &self,
        file: &HeldFile<N>,
        old: &RecordRead<N>,
    ) -> Result<(), ClaimError> {
        if N != 128 {
            return Err(ClaimError::invalid("entry_coordinator_record_size"));
        }
        self.validate_retained(file)?;
        if matches!(
            (self.guard.proof.origin, old),
            (ElectionRecordOrigin::Fresh(FreshRecordOrigin::Missing), RecordRead::Missing)
                | (ElectionRecordOrigin::Fresh(FreshRecordOrigin::Empty), RecordRead::Empty)
        ) {
            Ok(())
        } else {
            Err(ClaimError::invalid("entry_changed_fresh_record"))
        }
    }
    pub(crate) fn validate_retained<const N: usize>(
        &self,
        file: &HeldFile<N>,
    ) -> Result<(), ClaimError> {
        if N != 128 {
            return Err(ClaimError::invalid("entry_coordinator_record_size"));
        }
        if !std::ptr::eq(
            std::ptr::from_ref(file).cast::<()>(),
            std::ptr::from_ref(self.coordinator).cast::<()>(),
        ) || !self.claim_admission.load(Ordering::Acquire)
        {
            return Err(ClaimError::invalid("entry_changed_claim_lifetime"));
        }
        self.guard.check_coordinator(file, self.coordinator_inode)
    }
    pub(crate) fn binding(&self) -> Result<FreshPublicationBinding, ClaimError> {
        self.validate_retained(self.coordinator)?;
        Ok(FreshPublicationBinding {
            election: self.guard.proof.retained(),
            coordinator_inode: self.coordinator_inode,
            claim_origin: self.claim_origin.clone(),
            claim_admission: self.claim_admission.clone(),
        })
    }
}
pub(crate) struct FreshPublicationBinding {
    election: RetainedElectionProof,
    coordinator_inode: [u8; 24],
    claim_origin: Arc<()>,
    claim_admission: Arc<AtomicBool>,
}
impl FreshPublicationBinding {
    pub(crate) fn execution_owner(&self) -> ExecutionId {
        self.election.execution
    }
    pub(crate) fn validate<const N: usize>(
        &self,
        file: &HeldFile<N>,
        current: &FreshCoordinatorPublication<'_>,
    ) -> Result<(), ClaimError> {
        if N != 128 {
            return Err(ClaimError::invalid("entry_coordinator_record_size"));
        }
        let original = &self.election;
        let next = current.guard.proof;
        if !Arc::ptr_eq(&original.lifetime, &next.lifetime)
            || !Arc::ptr_eq(&self.claim_origin, &current.claim_origin)
            || !Arc::ptr_eq(&self.claim_admission, &current.claim_admission)
            || original.execution != next.execution
            || original.namespace != next.namespace
            || original.installation_identity != next.installation_identity
            || original.account_sid != next.account_sid
            || original.entry_inode != next.entry_inode
            || original.budget != next.budget
            || original.origin != next.origin
            || self.coordinator_inode != current.coordinator_inode
        {
            return Err(ClaimError::invalid("changed_publication_authority"));
        }
        current.validate_retained(file)
    }
}

#[cfg(test)]
pub(crate) mod fixture;
