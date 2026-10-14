// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded protected diagnostic custody across finite execution operations.
//!
//! Synchronous methods run only on admitted execution workers. Native handles and
//! partial stages remain in this owner outside cancellable result lifetimes.
//! Dropping a lease never closes native custody or proves durable disposition.
mod access;
use access::StateAccess;
mod activation;
mod allocation;
pub use allocation::*;
mod boundary;
pub(crate) use boundary::Port;
pub(crate) use boundary::SummaryStageAccess;
pub(crate) mod genesis;
pub use genesis::{
    DiagnosticStoreGenesis, DiagnosticStoreOpening, ProtectedDiagnosticWriteRequest,
};
mod finished_owner;
pub use finished_owner::{
    FINISHED_OWNER_STORAGE_BYTES, FINISHED_RUST_STORAGE_BYTES, ProtectedFinishedOwnerAllocation,
    ProtectedFinishedRustRetention,
};
mod capture_read;
pub use capture_read::{ProtectedCaptureReadGuard, ProtectedCaptureReadLease};
mod capture_intent;
pub use capture_intent::{
    CAPTURE_RETIREMENT_ERROR_BYTES, DiagnosticCaptureFence, DiagnosticCaptureIntent,
    DiagnosticCaptureNativeSource, DiagnosticCaptureRetirementError,
};
mod deletion;
mod disposition;
#[cfg(any(test, feature = "boundary-fixtures"))]
pub mod fixture;
pub(crate) mod inventory;
mod operation;
mod payload_discard;
mod port;
pub use payload_discard::*;
mod read;
mod recovery;
use operation::{check, next_lease};
mod retirement;
mod source;
mod stage;
mod summary_migration;
#[cfg(test)]
mod tests;
mod types;

pub use activation::DiagnosticActivation;
pub use deletion::{DiagnosticDeleteLease, DiagnosticDeletion, DurableDeletionPermit};
pub use disposition::{DiagnosticDispositionPlan, DispositionIntent, RecordedDiagnosticFileFacts};
pub use inventory::{
    DiagnosticEntryClass, DiagnosticInventory, DiagnosticInventoryEntry, DiagnosticInventoryStatus,
};
pub use read::DiagnosticReadLease;
pub use recovery::*;
pub(crate) use retirement::DiagnosticNativeRetirement;
pub use retirement::DiagnosticStorageSnapshot;
pub use stage::{DiagnosticDispositionStage, DiagnosticStage, DiagnosticStageFacts};
pub use summary_migration::*;
pub use types::*;

use crate::ClaimError;
use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};
use sugarsubstitute_contracts::{
    execution::{ExecutionId, JobContext, MemoryClaim, Moment, ScopeId},
    incident::LogicalStoreOperationKey,
    lifecycle::{CoordinatorKey, NamespaceId},
};
use types::DiagnosticStorageFailureCode as Code;

#[derive(Clone)]
struct Shared {
    inner: Arc<Mutex<State>>,
    _allocation: allocation::DiagnosticAllocationOwner,
}
impl Shared {
    fn ptr_eq(left: &Self, right: &Self) -> bool {
        Arc::ptr_eq(&left.inner, &right.inner)
    }
    fn check(
        &self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<StateAccess<'_>, DiagnosticStorageError> {
        if !Self::ptr_eq(self, &operation.shared) {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let mut access = StateAccess::acquire(self, operation.key);
        access.check(operation.key, context, false)?;
        Ok(access)
    }
}
impl std::ops::Deref for Shared {
    type Target = Mutex<State>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
pub(crate) type NativeId = u64;
/// One constructor-private operation binding, including its original deadline.
pub struct DiagnosticStorageOperation {
    shared: Shared,
    key: LogicalStoreOperationKey,
    deadline: Moment,
}
impl DiagnosticStorageOperation {
    /// Observe the complete current operation without granting filesystem authority.
    pub const fn key(&self) -> LogicalStoreOperationKey {
        self.key
    }
    /// Observe its first admitted monotonic deadline; later calls cannot extend it.
    pub const fn original_deadline(&self) -> Moment {
        self.deadline
    }
}

struct Binding {
    context: JobContext,
    key: LogicalStoreOperationKey,
    execution: ExecutionId,
    scope: ScopeId,
    deadline: Moment,
    cleanup: Option<Moment>,
    cleanup_scope: Option<ScopeId>,
    cleanup_context: Option<JobContext>,
    last_observed: Moment,
}
pub(crate) use port::OwnedPort;
struct State {
    port: Option<OwnedPort>,
    active_access: bool,
    retiring_handles: usize,
    retiring_files: usize,
    owner: Option<CoordinatorKey>,
    execution: Option<ExecutionId>,
    last_observed: Option<Moment>,
    high_water: u64,
    operation: Option<Binding>,
    namespace: Option<NamespaceId>,
    writer: bool,
    genesis: Option<genesis::RetainedGenesis>,
    inventory_complete: bool,
    charged_bytes: u64,
    payload_groups: Vec<[u8; 32]>,
    seeds: Vec<[u8; 32]>,
    summaries: Vec<[u8; 32]>,
    closed: bool,
    stages: Vec<stage::RetainedStage>,
    reads: Vec<read::RetainedRead>,
    capture_reads: usize,
    finished_owner: Option<u64>,
    capture: Option<capture_intent::RetainedCapture>,
    deletion: Option<deletion::RetainedDeletion>,
    migration: Option<allocation::RetainedIntent<summary_migration::MigrationIntent>>,
    payload: Option<allocation::RetainedIntent<payload_discard::intent::PayloadIntent>>,
    archives: Vec<summary_migration::MigrationArchive>,
    scan_generation: u64,
    scan_entries: Vec<inventory::ObservedEntry>,
    next_lease: u64,
    _scan_allocation: Option<allocation::DiagnosticAllocationLoan>,
    allocation: allocation::DiagnosticAllocationOwner,
}

/// Own one bounded account-protected store independently of a failed desktop.
pub struct DiagnosticStorage {
    shared: Shared,
}
impl DiagnosticStorage {
    /// Retain the complete8MiB claim before any filesystem effect.
    /// # Errors
    /// Rejects insufficient claim or bounded owner-state allocation failure.
    pub fn new(memory: MemoryClaim) -> Result<Self, DiagnosticStorageError> {
        if memory.bytes() < DIAGNOSTIC_STORAGE_BYTES {
            return Err(DiagnosticStorageError::rejected(None, Code::Capacity));
        }
        let allocation = allocation::owner(memory)?;
        let port =
            crate::protected_storage::record_file::StreamingPort::admit(allocation.native_grant())
                .map_err(|cause| DiagnosticStorageError::native(None, Code::Admission, cause))?;
        Self::from_port(allocation, port)
    }
    fn from_port(
        allocation: allocation::DiagnosticAllocationOwner,
        port: OwnedPort,
    ) -> Result<Self, DiagnosticStorageError> {
        let reservation = allocation::reservation(&allocation, None)?;
        let mut stages = Vec::new();
        let mut reads = Vec::new();
        let mut archives = Vec::new();
        stages
            .try_reserve_exact(DIAGNOSTIC_STAGE_BOUND)
            .and_then(|()| reads.try_reserve_exact(DIAGNOSTIC_READ_BOUND))
            .and_then(|()| archives.try_reserve_exact(DIAGNOSTIC_MIGRATION_ARCHIVE_BOUND))
            .map_err(|cause| reservation.failed(cause))?;
        if stages.capacity() != DIAGNOSTIC_STAGE_BOUND
            || reads.capacity() != DIAGNOSTIC_READ_BOUND
            || archives.capacity() != DIAGNOSTIC_MIGRATION_ARCHIVE_BOUND
        {
            return Err(DiagnosticStorageError::rejected(None, Code::Capacity));
        }
        Ok(Self {
            shared: Shared {
                _allocation: allocation.clone(),
                inner: Arc::new(Mutex::new(State {
                    port: Some(port),
                    active_access: false,
                    retiring_handles: 0,
                    retiring_files: 0,
                    owner: None,
                    execution: None,
                    last_observed: None,
                    high_water: 0,
                    operation: None,
                    namespace: None,
                    writer: false,
                    genesis: None,
                    inventory_complete: false,
                    charged_bytes: 0,
                    payload_groups: Vec::new(),
                    seeds: Vec::new(),
                    summaries: Vec::new(),
                    closed: false,
                    stages,
                    reads,
                    capture_reads: 0,
                    finished_owner: None,
                    capture: None,
                    deletion: None,
                    migration: None,
                    payload: None,
                    archives,
                    scan_generation: 0,
                    scan_entries: Vec::new(),
                    next_lease: 1,
                    _scan_allocation: None,
                    allocation,
                })),
            },
        })
    }
    /// Establish complete identity and the original effective deadline before effects.
    /// # Errors
    /// Rejects foreign execution/coordinator, retired/replayed IDs, live predecessors,
    /// missing/elapsed deadline, cancellation or supplied-clock regression.
    pub fn begin_operation(
        &mut self,
        key: LogicalStoreOperationKey,
        context: &JobContext,
    ) -> Result<DiagnosticStorageOperation, DiagnosticStorageError> {
        let now = context.now();
        let cancelled = context.cancellation_requested();
        let first = context.clone();
        let mut state = StateAccess::acquire(&self.shared, key);
        if state.data_mut()?.closed {
            return Err(error(key, Code::Closed));
        }
        if state.data_mut()?.operation.is_some() {
            return Err(error(key, Code::Busy));
        }
        if state.data_mut()?.owner.is_some_and(|owner| owner != key.owner())
            || state.data_mut()?.execution.is_some_and(|owner| owner != context.owner_id())
            || key.id().get() <= state.data_mut()?.high_water
        {
            return Err(error(key, Code::InvalidIdentity));
        }
        let deadline = context.deadline().ok_or_else(|| error(key, Code::Admission))?;
        if state.data_mut()?.last_observed.is_some_and(|old| now < old) {
            state.data_mut()?.closed = true;
            return Err(error(key, Code::Deadline));
        }
        if now >= deadline {
            return Err(error(key, Code::Deadline));
        }
        if deadline.as_nanos() - now.as_nanos() > DIAGNOSTIC_OPERATION_NANOS {
            return Err(error(key, Code::Admission));
        }
        if cancelled {
            return Err(error(key, Code::Cancelled));
        }
        state.data_mut()?.owner = Some(key.owner());
        state.data_mut()?.execution = Some(context.owner_id());
        state.data_mut()?.last_observed = Some(now);
        state.data_mut()?.operation = Some(Binding {
            key,
            context: first,
            execution: context.owner_id(),
            scope: context.scope_id(),
            deadline,
            cleanup: None,
            cleanup_scope: None,
            cleanup_context: None,
            last_observed: now,
        });
        state.call_native(|port| port.bind_operation(context))?;
        let binding = allocation::loans::Binding::from_operation(
            state.data_mut()?.operation.as_ref().ok_or_else(|| error(key, Code::Closed))?,
        );
        let previous = state.data_mut()?.allocation.bind(binding)?;
        let operation = DiagnosticStorageOperation { shared: self.shared.clone(), key, deadline };
        drop(state);
        drop(previous);
        Ok(operation)
    }
    /// Complete an operation only after all access/stage/disposition custody settles.
    /// # Errors
    /// Rejects unretired handles/leases, uncertain effects or changed original context.
    pub fn finish_operation(
        &mut self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        if !state.data_mut()?.stages.is_empty()
            || !state.data_mut()?.reads.is_empty()
            || state.data_mut()?.capture_reads != 0
            || state.data_mut()?.finished_owner.is_some()
            || state.data_mut()?.deletion.is_some()
            || state.data_mut()?.migration.is_some()
            || state.data_mut()?.payload.is_some()
        {
            return Err(error(operation.key, Code::Busy));
        }
        state.data_mut()?.high_water = operation.key.id().get();
        let previous = state.data_mut()?.operation.take();
        let native = state.data_mut()?.allocation.finish_binding();
        drop(state);
        drop(previous);
        drop(native);
        Ok(())
    }
    /// Open/create only the approved namespace beneath a verified existing host root.
    /// # Errors
    /// Rejects foreign namespace, repeated resolution, path/account/security uncertainty.
    /// Partial native outputs remain with the owner for checked retirement.
    pub fn resolve(
        &mut self,
        root: &Path,
        namespace: NamespaceId,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        if namespace != operation.key.owner().namespace || state.data_mut()?.namespace.is_some() {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        state
            .call_native(|port| port.resolve(root, namespace, context))?
            .map_err(|cause| native(operation.key, Code::ForeignProtection, cause))?;
        state.data_mut()?.namespace = Some(namespace);
        Ok(())
    }
    /// Acquire fail-immediately writer exclusion on the stable protected inode.
    /// # Errors
    /// Rejects unresolved/foreign scope, another live writer or partial native failure.
    pub fn acquire_writer(
        &mut self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        if state.data_mut()?.namespace.is_none() {
            return Err(error(operation.key, Code::Closed));
        }
        state
            .call_native(|port| port.acquire_writer(context))?
            .map_err(|cause| native(operation.key, Code::Busy, cause))?;
        state.data_mut()?.writer = true;
        Ok(())
    }
    fn check(
        &self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<StateAccess<'_>, DiagnosticStorageError> {
        self.shared.check(operation, context)
    }
}
impl Drop for DiagnosticStorage {
    fn drop(&mut self) {
        if lock(&self.shared).live_handles() != 0 {
            // Native owners and their original claim outlive unobserved helper teardown.
            std::mem::forget(self.shared.clone());
        }
    }
}
fn lock(shared: &Shared) -> MutexGuard<'_, State> {
    shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
fn error(key: LogicalStoreOperationKey, code: Code) -> DiagnosticStorageError {
    DiagnosticStorageError::rejected(Some(key), code)
}
fn native(key: LogicalStoreOperationKey, code: Code, cause: ClaimError) -> DiagnosticStorageError {
    DiagnosticStorageError::native(Some(key), code, cause)
}
