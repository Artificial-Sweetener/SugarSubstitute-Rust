// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep successful entry exclusion borrowed throughout actual claims initialization.
use super::{
    namespace::{NamespaceLifetime, NativeState},
    native::check,
};
use crate::{
    ClaimError,
    native_claims::lock,
    protected_storage::{account::ResolvedNamespace, exclusion::HeldFile, record_file::RecordRead},
};
use std::sync::{Arc, MutexGuard, atomic::Ordering};
use sugarsubstitute_contracts::{
    coordinator_entry::EntrySetupBudget,
    execution::{ExecutionId, JobContext},
    lifecycle::NamespaceId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FreshRecordOrigin {
    Missing,
    Empty,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ElectionRecordOrigin {
    Fresh(FreshRecordOrigin),
    Released([u8; 128]),
}
pub(crate) struct RetainedElectionProof {
    pub(crate) lifetime: Arc<NamespaceLifetime>,
    pub(crate) execution: ExecutionId,
    pub(crate) namespace: NamespaceId,
    pub(crate) installation_identity: [u8; 24],
    pub(crate) account_sid: Vec<u8>,
    pub(crate) entry_inode: [u8; 24],
    pub(crate) budget: EntrySetupBudget,
    pub(crate) origin: ElectionRecordOrigin,
    pub(crate) claim_lifetime: Option<crate::claims::entry::ClaimAdmissionLifetime>,
}
impl RetainedElectionProof {
    pub(crate) fn retained(&self) -> Self {
        Self {
            lifetime: self.lifetime.clone(),
            execution: self.execution,
            namespace: self.namespace,
            installation_identity: self.installation_identity,
            account_sid: self.account_sid.clone(),
            entry_inode: self.entry_inode,
            budget: self.budget,
            origin: self.origin,
            claim_lifetime: self.claim_lifetime.clone(),
        }
    }
    pub(crate) fn validate_live(&self) -> Result<(), ClaimError> {
        if self.lifetime.open.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(ClaimError::invalid("entry_election_closed"))
        }
    }
    pub(crate) fn lock_for_initialization<'a>(
        &'a self,
        context: &'a JobContext,
        resolved: &ResolvedNamespace,
    ) -> Result<ElectionInitializationGuard<'a>, ClaimError> {
        check(context)?;
        self.validate_live()?;
        if context.owner_id() != self.execution
            || context.now() < self.budget.started()
            || context.now() >= self.budget.until()
            || context.deadline().is_none_or(|until| until > self.budget.until())
            || resolved.namespace != self.namespace
            || resolved.installation_identity != self.installation_identity
            || resolved.account_sid != self.account_sid
        {
            return Err(ClaimError::invalid("entry_initialization_binding"));
        }
        let state = lock(&self.lifetime.state);
        let guard = ElectionInitializationGuard { proof: self, state, context };
        guard.validate_retained()?;
        Ok(guard)
    }
}
pub(crate) struct ElectionInitializationGuard<'a> {
    pub(crate) proof: &'a RetainedElectionProof,
    state: MutexGuard<'a, NativeState>,
    pub(crate) context: &'a JobContext,
}
impl ElectionInitializationGuard<'_> {
    pub(crate) fn validate_retained(&self) -> Result<(), ClaimError> {
        check(self.context)?;
        self.proof.validate_live()?;
        if !self
            .proof
            .claim_lifetime
            .as_ref()
            .is_some_and(|claim| claim.admission.load(Ordering::Acquire))
        {
            return Err(ClaimError::invalid("entry_claim_admission_closed"));
        }
        if self.context.owner_id() != self.proof.execution
            || self.context.now() >= self.proof.budget.until()
        {
            return Err(ClaimError::invalid("entry_initialization_expired"));
        }
        let namespace = self
            .state
            .namespace
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("entry_retired_namespace"))?;
        if self.state.entry_inode != Some(self.proof.entry_inode)
            || namespace.namespace != self.proof.namespace
            || namespace.installation_identity != self.proof.installation_identity
            || namespace.account_sid != self.proof.account_sid
            || self.state.record_origin != Some(self.proof.origin)
        {
            return Err(ClaimError::invalid("entry_changed_election"));
        }
        self.proof.lifetime.port.validate_election(&self.state, self.proof, self.context)
    }
    pub(crate) fn check_record(&self, record: &RecordRead<128>) -> Result<(), ClaimError> {
        self.validate_retained()?;
        if matches!(
            (self.proof.origin, record),
            (ElectionRecordOrigin::Fresh(FreshRecordOrigin::Missing), RecordRead::Missing)
                | (ElectionRecordOrigin::Fresh(FreshRecordOrigin::Empty), RecordRead::Empty)
        ) || matches!((self.proof.origin,record),(ElectionRecordOrigin::Released(original),RecordRead::Present(actual)) if original==*actual)
        {
            Ok(())
        } else {
            Err(ClaimError::invalid("entry_changed_election_record"))
        }
    }
    pub(crate) fn check_coordinator<const N: usize>(
        &self,
        file: &HeldFile<N>,
        inode: [u8; 24],
    ) -> Result<(), ClaimError> {
        if N != 128 {
            return Err(ClaimError::invalid("entry_coordinator_record_size"));
        }
        self.validate_retained()?;
        let namespace = self
            .state
            .namespace
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("entry_namespace_missing"))?;
        let expected = namespace
            .directory
            .join("coordinators")
            .join(crate::protected_storage::record_file::hex(self.proof.namespace.bytes()))
            .with_extension("record");
        if !file.is_locked()
            || file.record_path != expected
            || file.account_sid != self.proof.account_sid
        {
            return Err(ClaimError::invalid("entry_coordinator_exclusion_binding"));
        }
        self.proof.lifetime.port.validate_coordinator(
            &self.state,
            &file.handle,
            inode,
            self.context,
        )
    }
}
