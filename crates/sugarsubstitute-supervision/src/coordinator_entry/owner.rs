// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit finite work and consume exact outcomes before publishing cached entry proof.
use super::{
    CoordinatorEntryInput, ENTRY_PREPARATION_BYTES,
    election::{ElectionRecordOrigin, RetainedElectionProof},
    namespace::{NamespaceLifetime, VerifiedEntryNamespace},
    native::{self, Discovered, Elected, NativePort},
    types::*,
};
use crate::{
    ClaimError, WindowsProcessRegistry, WindowsResourceClaims,
    claims::entry::RetainedCoordinatorProof, native_claims::lock,
};
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock, atomic::Ordering},
};
use sugarsubstitute_contracts::{
    coordinator_entry::EntrySetupBudget,
    execution::{
        CompletionBatch, ExecutionId, ExecutionPort, JobClass, JobId, JobOutcome, JobSpec, Moment,
        Operation, OwnedJob, Scope,
    },
    lifecycle::{CoordinatorEpoch, CoordinatorKey},
};
use sugarsubstitute_execution::{
    CoordinatorEntryClientInput, CoordinatorEntryServerInput, EntryRequestInput, ExecutionClient,
    VerifiedEntryPeer,
};

mod admission;
mod completion;
#[cfg(test)]
mod tests;

struct Paths {
    installation: PathBuf,
    executable: PathBuf,
    preparation_memory: OnceLock<Arc<sugarsubstitute_contracts::execution::MemoryClaim>>,
}
enum Actual {
    Discovered(Discovered),
    Elected(Elected),
    Closed,
}
struct Pending {
    job: OwnedJob<Actual, ClaimError>,
    cleanup: bool,
}
/// Own original setup, finite jobs, atomic proof fences and actual native custody.
pub struct CoordinatorEntryOwner {
    paths: Option<Arc<Paths>>,
    request: Option<EntryRequestInput>,
    budget: EntrySetupBudget,
    lifetime: Arc<NamespaceLifetime>,
    port: Arc<dyn NativePort>,
    execution: Option<ExecutionId>,
    client: Option<ExecutionClient>,
    pending: Option<Pending>,
    discovered: Option<Discovered>,
    candidate: Option<CoordinatorEpoch>,
    election: Option<RetainedElectionProof>,
    coordinator: Option<RetainedCoordinatorProof>,
    owner: Option<CoordinatorKey>,
    namespace_taken: bool,
    close_deadline: Option<Moment>,
    phase: EntryPhase,
    events: [Option<EntryObservation>; 4],
}
impl CoordinatorEntryOwner {
    /// Retain bounded inert input and original setup without native effects.
    pub fn new(input: CoordinatorEntryInput, budget: EntrySetupBudget) -> Self {
        Self::with_port(input, budget, Arc::new(native::SystemPort))
    }
    pub(super) fn with_port(
        input: CoordinatorEntryInput,
        budget: EntrySetupBudget,
        port: Arc<dyn NativePort>,
    ) -> Self {
        Self {
            paths: Some(Arc::new(Paths {
                installation: input.installation,
                executable: input.executable,
                preparation_memory: OnceLock::new(),
            })),
            request: input.request,
            budget,
            lifetime: NamespaceLifetime::new(port.clone()),
            port,
            execution: None,
            client: None,
            pending: None,
            discovered: None,
            candidate: None,
            election: None,
            coordinator: None,
            owner: None,
            namespace_taken: false,
            close_deadline: None,
            phase: EntryPhase::Inert,
            events: std::array::from_fn(|_| None),
        }
    }
    /// Return the unchanged original setup budget as inert matching data.
    /// Native authority and current-time validation remain with the retained owner.
    pub const fn original_setup_budget(&self) -> EntrySetupBudget {
        self.budget
    }
    /// Return the actual execution identity retained after admitted discovery.
    /// This cached identity grants no native authority or scheduler capability.
    pub const fn original_execution_owner(&self) -> Option<ExecutionId> {
        self.execution
    }
    /// Observe cached immutable metadata without native access or mutex locking.
    pub fn snapshot(&self) -> EntrySnapshot {
        EntrySnapshot {
            phase: self.phase,
            namespace: self.discovered.map(|known| known.facts.namespace()),
            owner: self.owner,
            pending: self.pending.as_ref().map(|pending| pending.job.spec().id),
        }
    }
    /// Transfer the actual consumed namespace proof once; its shared custody remains retained.
    pub fn take_namespace(&mut self) -> Option<VerifiedEntryNamespace> {
        if self.namespace_taken || !self.lifetime.open.load(Ordering::Acquire) {
            return None;
        }
        let known = self.discovered?;
        let execution = self.execution?;
        self.namespace_taken = true;
        Some(VerifiedEntryNamespace {
            lifetime: self.lifetime.clone(),
            facts: known.facts,
            installation_identity: known.installation_identity,
            execution,
            budget: self.budget,
        })
    }
    /// Bind current retained election metadata to an unused claims owner without native work.
    /// A recovered Held predecessor retains both exclusions; initialization moves
    /// the same coordinator file while its entry guard remains borrowed through CAS.
    /// # Errors
    /// Rejects absent/closed/foreign election or actual claims admission failure.
    pub fn prepare_claims(
        &self,
        claims: &mut WindowsResourceClaims,
    ) -> Result<(), EntryAdmissionError> {
        self.current()?;
        let proof = self.election.as_ref().ok_or(EntryAdmissionError::Identity)?;
        claims.bind_entry_initialization(proof.retained()).map_err(EntryAdmissionError::Claims)
    }
    /// Produce inert discovery server facts from actual live namespace/election proof.
    /// # Errors
    /// Rejects absent/closed election; no coordinator authority is granted.
    pub fn initializing_server_input(
        &self,
    ) -> Result<CoordinatorEntryServerInput, EntryAdmissionError> {
        self.current()?;
        self.election
            .as_ref()
            .ok_or(EntryAdmissionError::Identity)?
            .validate_live()
            .map_err(EntryAdmissionError::Native)?;
        CoordinatorEntryServerInput::new(
            self.discovered.ok_or(EntryAdmissionError::Identity)?.facts,
        )
        .map_err(|cause| EntryAdmissionError::Native(ClaimError::new("entry_server_input", cause)))
    }
    /// Bind actual successful current claims and exact registry before Serving admission.
    /// # Errors
    /// Rejects absent/closed/foreign native proof or incomplete actual claims acquisition.
    pub fn bind_coordinator(
        &mut self,
        claims: &WindowsResourceClaims,
        registry: &WindowsProcessRegistry,
        owner: CoordinatorKey,
    ) -> Result<(), EntryAdmissionError> {
        self.current()?;
        if self.owner.is_some()
            || self.coordinator.is_some()
            || owner.namespace
                != self.discovered.ok_or(EntryAdmissionError::Identity)?.facts.namespace()
            || self.election.is_none()
        {
            return Err(EntryAdmissionError::Identity);
        }
        let execution = self.execution.ok_or(EntryAdmissionError::Identity)?;
        let proof = claims
            .retained_coordinator(registry, owner, execution)
            .map_err(EntryAdmissionError::Claims)?;
        if !proof.belongs_to(claims) {
            return Err(EntryAdmissionError::Identity);
        }
        proof.validate_registry(registry).map_err(EntryAdmissionError::Claims)?;
        self.coordinator = Some(proof);
        self.owner = Some(owner);
        self.phase = EntryPhase::Serving;
        Ok(())
    }
    /// Move the original request exactly once into inert client transport input.
    /// # Errors
    /// Rejects missing namespace/request, expired original setup or counter conversion failure.
    pub fn client_input(&mut self) -> Result<CoordinatorEntryClientInput, EntryAdmissionError> {
        self.current()?;
        let known = self.discovered.ok_or(EntryAdmissionError::Identity)?;
        let client = self.client.as_ref().ok_or(EntryAdmissionError::Identity)?;
        let source = client
            .encode_prepared_entry_deadline(
                self.request.as_ref().ok_or(EntryAdmissionError::Closed)?,
                self.budget,
            )
            .map_err(EntryAdmissionError::PreparedTransport)?;
        let request = self.request.take().ok_or(EntryAdmissionError::Closed)?;
        CoordinatorEntryClientInput::from_owned(known.facts, self.candidate, request, source)
            .map_err(|cause| {
                EntryAdmissionError::Native(ClaimError::new("entry_client_input", cause))
            })
    }
    /// Validate current actual cached namespace/election/claim and authenticated peer facts.
    /// # Errors
    /// Rejects revoked or foreign proof; caller still owns final native launch admission.
    pub fn validate_current_owner(
        &self,
        owner: CoordinatorKey,
        peer: &VerifiedEntryPeer,
    ) -> Result<(), EntryAdmissionError> {
        self.current_serving()?;
        let known = self.discovered.ok_or(EntryAdmissionError::Identity)?.facts;
        let execution = self.execution.ok_or(EntryAdmissionError::Identity)?;
        if self.owner != Some(owner)
            || peer.namespace() != known.namespace()
            || peer.account_sid() != known.account_sid()
            || peer.session_id() != known.session_id()
            || peer.host_image_identity() != known.host_image_identity()
        {
            return Err(EntryAdmissionError::Identity);
        }
        self.election
            .as_ref()
            .ok_or(EntryAdmissionError::Identity)?
            .validate_live()
            .map_err(EntryAdmissionError::Native)?;
        self.coordinator
            .as_ref()
            .ok_or(EntryAdmissionError::Identity)?
            .validate_live(owner, execution)
            .map_err(EntryAdmissionError::Claims)
    }
    /// Invalidate admission before cancellation or any actual native retirement.
    pub fn close_admission(&mut self) {
        self.lifetime.close_admission();
        self.request = None;
        self.paths = None;
        if let Some(pending) = &self.pending
            && !pending.cleanup
        {
            pending.job.cancel();
        }
        if self.phase != EntryPhase::Settled {
            self.phase = EntryPhase::Closing;
        }
    }
    fn current(&self) -> Result<(), EntryAdmissionError> {
        self.current_serving()?;
        let now = self
            .client
            .as_ref()
            .ok_or(EntryAdmissionError::Identity)?
            .now()
            .map_err(EntryAdmissionError::Clock)?;
        if now < self.budget.started() || now >= self.budget.until() {
            return Err(EntryAdmissionError::Closed);
        }
        Ok(())
    }
    fn current_serving(&self) -> Result<(), EntryAdmissionError> {
        if self.lifetime.open.load(Ordering::Acquire) && self.phase != EntryPhase::Settled {
            Ok(())
        } else {
            Err(EntryAdmissionError::Closed)
        }
    }
}
impl Drop for CoordinatorEntryOwner {
    fn drop(&mut self) {
        self.close_admission();
    }
}
