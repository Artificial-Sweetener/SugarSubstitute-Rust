// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual native claims before typed delivery and bind release to owned families.
#[cfg(feature = "boundary-fixtures")]
pub mod boundary_fixture;
mod completion;
pub(crate) mod entry;
mod launch;
pub(crate) mod managed_custody;
pub(crate) mod pool;
mod release;
mod submission;
#[cfg(test)]
mod tests;
use crate::{
    ClaimAdmissionError, ClaimError, ClaimObservation, ClaimTarget, ResolutionTicket,
    native_claims::{self, Backend, NativePort, Resolution, Root},
    registry::RegistryIdentity,
};
pub(crate) use native_claims::lock;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
};
use sugarsubstitute_contracts::{
    configuration::LaunchConfiguration,
    execution::{CompletionBatch, ExecutionPort, JobId, JobSpec, OwnedJob, Scope},
    lifecycle::{CoordinatorKey, ResourceKey},
};
use sugarsubstitute_execution::ExecutionClient;

/// Minimum transient reservation for bounded native descriptor/path scratch, in bytes.
pub const CLAIM_OPERATION_BYTES: u64 = 256 * 1024;
struct Shared<T> {
    value: Mutex<T>,
    failure: Mutex<Option<ClaimError>>,
}
impl<T> Shared<T> {
    fn new(value: T) -> Arc<Self> {
        Arc::new(Self { value: Mutex::new(value), failure: Mutex::new(None) })
    }
    fn fail(&self, cause: ClaimError) -> ClaimError {
        let mut failure = lock(&self.failure);
        if failure.is_none() {
            *failure = Some(cause.clone());
        }
        cause
    }
}
struct ResolutionEntry {
    ticket: ResolutionTicket,
    shared: Arc<Shared<Option<Resolution>>>,
    proof: Option<Arc<Resolution>>,
    pending: bool,
    failed: bool,
}
enum LaunchState {
    Unattempted,
    Accepted(crate::LaunchTicket),
    Rejected,
}
struct BackendEntry {
    resource: ResourceKey,
    shared: Arc<Shared<Backend>>,
    proof: Arc<Resolution>,
    pending: bool,
    claimed: bool,
    prepared: Arc<AtomicU8>,
    launch: LaunchState,
    registry: Option<RegistryIdentity>,
    failed: bool,
}
#[derive(Clone, Copy)]
enum Action {
    Initialize,
    Resolve(usize),
    Claim(usize),
    Retire(usize),
    Release(usize),
    Close,
}
struct Pending {
    job: OwnedJob<(), ClaimError>,
    action: Action,
    target: ClaimTarget,
}
struct Events {
    entries: Box<[Option<ClaimObservation>]>,
    head: usize,
    length: usize,
}
impl Events {
    fn new() -> Result<Self, ClaimError> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(512)
            .map_err(|cause| ClaimError::new("claim_events_allocation", cause))?;
        entries.resize_with(512, || None);
        Ok(Self { entries: entries.into_boxed_slice(), head: 0, length: 0 })
    }
    fn push(&mut self, value: ClaimObservation) {
        // Admission reserves three event positions per pending finite operation.
        let index = (self.head + self.length) % self.entries.len();
        self.entries[index] = Some(value);
        self.length += 1;
    }
    fn pop(&mut self) -> Option<ClaimObservation> {
        if self.length == 0 {
            return None;
        }
        let result = self.entries[self.head].take();
        self.head = (self.head + 1) % self.entries.len();
        self.length -= 1;
        result
    }
}

/// Own one account/installation claim generation and at most64 retained backend proofs.
/// All filesystem effects use finite jobs submitted through the execution owner.
/// Cancellation stops admission; it does not release acquired exclusions or families.
pub struct WindowsResourceClaims {
    origin: Arc<()>,
    root: Arc<Shared<Root>>,
    port: Arc<dyn NativePort>,
    open: Arc<AtomicBool>,
    initialized: bool,
    entry_initialized_held: Arc<AtomicBool>,
    entry_initialization: Option<crate::coordinator_entry::election::RetainedElectionProof>,
    initialization_started: bool,
    initialization_pending: bool,
    closed: bool,
    next_resolution: u64,
    owner: Option<CoordinatorKey>,
    registry: Option<RegistryIdentity>,
    custodian: Option<crate::BackendCustodyParent>,
    execution_owner: Option<sugarsubstitute_contracts::execution::ExecutionId>,
    initial_scope: Option<sugarsubstitute_contracts::execution::ScopeId>,
    resolutions: [Option<ResolutionEntry>; 64],
    backends: [Option<BackendEntry>; 64],
    pending: [Option<Pending>; 12],
    events: Events,
}
impl WindowsResourceClaims {
    /// Create native-free state with a checked, fixed-capacity heap observation table.
    /// Allocation failure retains its original cause. Initialization must complete
    /// before new resolutions; no native ownership exists on construction failure.
    pub fn new() -> Result<Self, ClaimError> {
        let events = Events::new()?;
        Ok(Self {
            origin: Arc::new(()),
            root: Shared::new(Root::new()),
            port: Arc::new(native_claims::SystemPort),
            open: Arc::new(AtomicBool::new(true)),
            initialized: false,
            entry_initialized_held: Arc::new(AtomicBool::new(false)),
            entry_initialization: None,
            initialization_started: false,
            initialization_pending: false,
            closed: false,
            next_resolution: 1,
            owner: None,
            registry: None,
            custodian: None,
            execution_owner: None,
            initial_scope: None,
            resolutions: std::array::from_fn(|_| None),
            backends: std::array::from_fn(|_| None),
            pending: std::array::from_fn(|_| None),
            events,
        })
    }
    /// Stop new initialization, resolution, claim and launch admission without discarding work.
    pub fn close_admission(&mut self) {
        self.entry_initialized_held.store(false, Ordering::Release);
        self.open.store(false, Ordering::Release);
    }
    /// Bind the one process owner after actual coordinator acquisition, without native effects.
    /// A foreign coordinator or repeated binding is rejected, including the same registry twice.
    pub fn bind_process_registry(
        &mut self,
        registry: &crate::WindowsProcessRegistry,
    ) -> Result<(), ClaimAdmissionError> {
        if self.closed || self.initialization_pending || self.registry.is_some() {
            return Err(ClaimAdmissionError::Identity);
        }
        let owner = self.owner.ok_or(ClaimAdmissionError::Identity)?;
        let identity = registry.claim_identity(owner).map_err(ClaimAdmissionError::Registry)?;
        self.registry = Some(identity);
        Ok(())
    }
    /// Bind one actual authenticated witness before any backend generation is acquired.
    /// Matching IDs alone cannot bind another execution's transport or renew this source.
    /// Existing pending/owned generations prevent changing their original custodian.
    /// # Errors
    /// Preserves original admission/provider failures and rejects foreign or repeated sources.
    pub fn bind_backend_custodian(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        parent: &crate::BackendCustodyParent,
    ) -> Result<(), ClaimAdmissionError> {
        self.usable()?;
        self.validate_scope(scope)?;
        if self.custodian.is_some()
            || self.backends.iter().any(Option::is_some)
            || self.owner != Some(parent.key().owner())
            || !parent.matches_execution(execution)
        {
            return Err(ClaimAdmissionError::Identity);
        }
        parent.check().map_err(|cause| {
            ClaimAdmissionError::Native(ClaimError::new("custodian_binding", cause))
        })?;
        self.custodian = Some(parent.clone());
        Ok(())
    }
    /// Transfer a cached actual observation; this method performs no native work.
    pub fn take_observation(&mut self) -> Option<ClaimObservation> {
        self.events.pop()
    }
    /// Identify the original admitted pool scope, whose byte charge survives typed acknowledgement.
    pub const fn initialization_scope(
        &self,
    ) -> Option<sugarsubstitute_contracts::execution::ScopeId> {
        self.initial_scope
    }
    /// Borrow a successfully retained canonical configuration; this grants no handles.
    /// Rejects another owner, pending work, failed resolution or retired identity.
    pub fn resolved_configuration(
        &self,
        ticket: &ResolutionTicket,
    ) -> Result<&LaunchConfiguration, ClaimAdmissionError> {
        let index = self.resolution_index(ticket)?;
        let entry = self.resolutions[index].as_ref().ok_or(ClaimAdmissionError::Identity)?;
        if entry.pending || entry.failed {
            return Err(ClaimAdmissionError::Busy);
        }
        Ok(&entry.proof.as_ref().ok_or(ClaimAdmissionError::Busy)?.configuration)
    }
    /// Borrow the existing native backend identity hash after successful resolution.
    ///
    /// Root/interpreter aliases and casing share their actual native object identity. The
    /// original account, exact arguments and port remain bound. This correlation value grants
    /// no handles or process authority and must travel only with authenticated current facts.
    /// # Errors
    /// Rejects foreign, pending, failed or retired resolution tickets.
    pub fn resolved_backend_binding(
        &self,
        ticket: &ResolutionTicket,
    ) -> Result<sugarsubstitute_contracts::lifecycle::BackendBindingDigest, ClaimAdmissionError>
    {
        self.resolved_configuration(ticket)?;
        let entry = self.resolutions[self.resolution_index(ticket)?]
            .as_ref()
            .ok_or(ClaimAdmissionError::Identity)?;
        let proof = entry.proof.as_ref().ok_or(ClaimAdmissionError::Busy)?;
        Ok(sugarsubstitute_contracts::lifecycle::BackendBindingDigest::from_bytes(proof.digest))
    }
    fn resolution_index(&self, ticket: &ResolutionTicket) -> Result<usize, ClaimAdmissionError> {
        if !Arc::ptr_eq(&self.origin, &ticket.origin) {
            return Err(ClaimAdmissionError::Identity);
        }
        self.resolutions
            .iter()
            .position(|entry| entry.as_ref().is_some_and(|entry| entry.ticket.id == ticket.id))
            .ok_or(ClaimAdmissionError::Identity)
    }
    fn backend_index(&self, resource: ResourceKey) -> Result<usize, ClaimAdmissionError> {
        if Some(resource.owner) != self.owner {
            return Err(ClaimAdmissionError::Identity);
        }
        self.backends
            .iter()
            .position(|entry| entry.as_ref().is_some_and(|entry| entry.resource == resource))
            .ok_or(ClaimAdmissionError::Identity)
    }
    fn admit_index(
        &self,
        spec: JobSpec,
        initialization: bool,
    ) -> Result<usize, ClaimAdmissionError> {
        use sugarsubstitute_contracts::execution::{JobClass, Operation};
        if spec.class != JobClass::InfrastructureLifecycle
            || spec.operation != Operation::NativeLifecycle
            || spec.replacement.is_some()
            || spec.reservation_bytes
                < if initialization { crate::CLAIM_POOL_BYTES } else { CLAIM_OPERATION_BYTES }
            || (initialization && spec.reservation_bytes != crate::CLAIM_POOL_BYTES)
        {
            return Err(ClaimAdmissionError::Specification);
        }
        if self.events.length + 3 * (self.pending.iter().flatten().count() + 1) > 512 {
            return Err(ClaimAdmissionError::Saturated);
        }
        self.pending.iter().position(Option::is_none).ok_or(ClaimAdmissionError::Saturated)
    }
    fn usable(&self) -> Result<(), ClaimAdmissionError> {
        if !self.open.load(Ordering::Acquire) || !self.initialized || self.closed {
            Err(ClaimAdmissionError::Closed)
        } else {
            Ok(())
        }
    }
    fn validate_scope(&self, scope: &Scope) -> Result<(), ClaimAdmissionError> {
        if self.execution_owner.is_some_and(|owner| owner != scope.owner_id()) {
            Err(ClaimAdmissionError::Identity)
        } else {
            Ok(())
        }
    }
}
impl Drop for WindowsResourceClaims {
    fn drop(&mut self) {
        self.entry_initialized_held.store(false, Ordering::Release);
        self.open.store(false, Ordering::Release);
        for pending in self.pending.iter().flatten() {
            pending.job.cancel();
        }
        // Actual native state remains captured by every admitted worker until return.
        // Active generation files intentionally survive unobserved owner loss.
    }
}
