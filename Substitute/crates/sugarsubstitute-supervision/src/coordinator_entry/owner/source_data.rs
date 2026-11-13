// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the actual elected coordinator's revoked-at-close source settlement admission.
use super::*;
use sugarsubstitute_contracts::{
    coordinator_entry::{EntryOriginalsReference, EntryRequestId},
    execution::{CounterDeadline, JobContext},
};
use sugarsubstitute_execution::AuthenticatedEntryLaunch;

/// Purpose-specific source-data settlement under one actual authenticated invocation.
///
/// This opaque owner retains existing namespace/election/coordinator fences; it grants no
/// SelectionStore, managed backend, process launch or old diagnostic writer capability.
pub struct EntrySourceDataAdmission {
    lifetime: Arc<NamespaceLifetime>,
    coordinator: RetainedCoordinatorProof,
    owner: CoordinatorKey,
    execution: ExecutionId,
    peer: VerifiedEntryPeer,
    request: EntryRequestId,
    source: CounterDeadline,
    received: Moment,
    until: Moment,
    reference: Option<EntryOriginalsReference>,
    image_directory: [u8; 24],
}
impl CoordinatorEntryOwner {
    /// Mint only after the actual serving/elected owner and kernel-authenticated peer agree.
    /// No native effect, blocking mutex or file lookup runs on this composing thread.
    /// # Errors
    /// Rejects closed/foreign actual ownership, malformed body or unavailable retained proofs.
    pub fn source_data_admission(
        &self,
        owner: CoordinatorKey,
        request: &AuthenticatedEntryLaunch,
    ) -> Result<EntrySourceDataAdmission, EntryAdmissionError> {
        self.validate_current_owner(owner, request.peer())?;
        let view = request.view().map_err(|cause| {
            EntryAdmissionError::Native(ClaimError::new("entry_source_data_body", cause))
        })?;
        Ok(EntrySourceDataAdmission {
            lifetime: self.lifetime.clone(),
            coordinator: self.coordinator.as_ref().ok_or(EntryAdmissionError::Identity)?.clone(),
            owner,
            execution: self.execution.ok_or(EntryAdmissionError::Identity)?,
            peer: *request.peer(),
            request: request.request_id(),
            source: request.source_deadline(),
            received: request.received_at(),
            until: request.until(),
            reference: view.originals(),
            image_directory: self
                .discovered
                .ok_or(EntryAdmissionError::Identity)?
                .installation_identity,
        })
    }
}
impl EntrySourceDataAdmission {
    /// Recheck the exact frame's original context and all retained revocation fences.
    /// # Errors
    /// Rejects another execution/request/peer/source/cutoff, closure, cancellation or expiry.
    pub(crate) fn validate(
        &self,
        request: &AuthenticatedEntryLaunch,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        crate::coordinator_entry::native::check(context)?;
        if !self.lifetime.open.load(Ordering::Acquire)
            || self.lifetime.retired.load(Ordering::Acquire)
            || context.owner_id() != self.execution
            || context.now() < self.received
            || context.now() >= self.until
            || context.deadline().is_none_or(|until| until > self.until)
            || *request.peer() != self.peer
            || request.request_id() != self.request
            || request.source_deadline() != self.source
            || request.until() != self.until
            || request.received_at() != self.received
            || request
                .view()
                .map_err(|cause| ClaimError::new("entry_source_data_body", cause))?
                .originals()
                != self.reference
        {
            return Err(ClaimError::invalid("entry_source_data_admission"));
        }
        self.coordinator
            .validate_live(self.owner, self.execution)
            .map_err(|cause| ClaimError::new("entry_source_data_owner", cause))
    }
    pub(crate) fn owner(&self) -> CoordinatorKey {
        self.owner
    }
    pub(crate) fn image_directory(&self) -> [u8; 24] {
        self.image_directory
    }
}
