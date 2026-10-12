// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded protected diagnostic custody across finite execution operations.
//!
//! Synchronous methods run only on admitted execution workers. Native handles and
//! partial stages remain in this owner outside cancellable result lifetimes.
//! Dropping a lease never closes native custody or proves durable disposition.
mod activation;
mod deletion;
mod disposition;
#[cfg(any(test, feature = "boundary-fixtures"))]
pub mod fixture;
pub(crate) mod inventory;
mod operation;
mod read;
mod recovery;
use operation::{check, next_lease};
mod retirement;
mod stage;
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
pub use retirement::DiagnosticStorageSnapshot;
pub use stage::{DiagnosticDispositionStage, DiagnosticStage, DiagnosticStageFacts};
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

type Shared = Arc<Mutex<State>>;
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
    key: LogicalStoreOperationKey,
    execution: ExecutionId,
    scope: ScopeId,
    deadline: Moment,
    cleanup: Option<Moment>,
    cleanup_scope: Option<ScopeId>,
    last_observed: Moment,
}
struct State {
    _memory: Arc<MemoryClaim>,
    port: Box<dyn Port>,
    owner: Option<CoordinatorKey>,
    execution: Option<ExecutionId>,
    last_observed: Option<Moment>,
    high_water: u64,
    operation: Option<Binding>,
    namespace: Option<NamespaceId>,
    writer: bool,
    inventory_complete: bool,
    charged_bytes: u64,
    payload_groups: Vec<[u8; 32]>,
    seeds: Vec<[u8; 32]>,
    summaries: Vec<[u8; 32]>,
    closed: bool,
    stages: Vec<stage::RetainedStage>,
    reads: Vec<read::RetainedRead>,
    deletion: Option<deletion::RetainedDeletion>,
    next_lease: u64,
}

pub(crate) trait Port: Send {
    fn resolve(
        &mut self,
        root: &Path,
        namespace: NamespaceId,
        context: &JobContext,
    ) -> Result<(), ClaimError>;
    fn acquire_writer(&mut self, context: &JobContext) -> Result<(), ClaimError>;
    fn scan(&mut self, context: &JobContext) -> Result<inventory::Scan, ClaimError>;
    fn open(
        &mut self,
        object: DiagnosticObject,
        bound: u64,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError>;
    fn open_inventory(
        &mut self,
        observed: &inventory::ObservedEntry,
        bound: u64,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError>;
    fn create_stage(
        &mut self,
        object: DiagnosticObject,
        operation: LogicalStoreOperationKey,
        reserved: u64,
        context: &JobContext,
    ) -> Result<NativeId, ClaimError>;
    fn recover_stage(
        &mut self,
        object: DiagnosticObject,
        historical: LogicalStoreOperationKey,
        expected: RecordedDiagnosticFileFacts,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError>;
    fn read(
        &mut self,
        id: NativeId,
        offset: u64,
        output: &mut [u8],
        context: &JobContext,
    ) -> Result<usize, ClaimError>;
    fn write(
        &mut self,
        id: NativeId,
        offset: u64,
        input: &[u8],
        context: &JobContext,
    ) -> Result<(), ClaimError>;
    fn flush(
        &mut self,
        id: NativeId,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError>;
    fn activate(
        &mut self,
        id: NativeId,
        object: DiagnosticObject,
        expected: DiagnosticPredecessor,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError>;
    fn reconcile_activation(
        &mut self,
        object: DiagnosticObject,
        expected: DiagnosticPredecessor,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError>;
    fn delete(
        &mut self,
        id: NativeId,
        object: DiagnosticObject,
        expected: DiagnosticReadFacts,
        context: &JobContext,
    ) -> Result<(), ClaimError>;
    fn absent(
        &mut self,
        object: DiagnosticObject,
        context: &JobContext,
    ) -> Result<bool, ClaimError>;
    fn close(&mut self, id: NativeId, context: &JobContext) -> Result<(), ClaimError>;
    fn retire(&mut self, context: &JobContext) -> Result<(), ClaimError>;
    fn live_handles(&self) -> usize;
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
        let memory = Arc::new(memory);
        let port = crate::protected_storage::record_file::StreamingPort::new(memory.clone())
            .map_err(|cause| DiagnosticStorageError::native(None, Code::Admission, cause))?;
        Self::from_port(memory, Box::new(port))
    }
    fn from_port(
        memory: Arc<MemoryClaim>,
        port: Box<dyn Port>,
    ) -> Result<Self, DiagnosticStorageError> {
        if memory.bytes() < DIAGNOSTIC_STORAGE_BYTES {
            return Err(DiagnosticStorageError::rejected(None, Code::Capacity));
        }
        let mut stages = Vec::new();
        let mut reads = Vec::new();
        stages
            .try_reserve_exact(DIAGNOSTIC_STAGE_BOUND)
            .and_then(|()| reads.try_reserve_exact(DIAGNOSTIC_READ_BOUND))
            .map_err(|cause| {
                DiagnosticStorageError::native(
                    None,
                    Code::Capacity,
                    ClaimError::new("diagnostic_owner_reservation", cause),
                )
            })?;
        Ok(Self {
            shared: Arc::new(Mutex::new(State {
                _memory: memory,
                port,
                owner: None,
                execution: None,
                last_observed: None,
                high_water: 0,
                operation: None,
                namespace: None,
                writer: false,
                inventory_complete: false,
                charged_bytes: 0,
                payload_groups: Vec::new(),
                seeds: Vec::new(),
                summaries: Vec::new(),
                closed: false,
                stages,
                reads,
                deletion: None,
                next_lease: 1,
            })),
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
        let mut state = lock(&self.shared);
        if state.closed {
            return Err(error(key, Code::Closed));
        }
        if state.operation.is_some() {
            return Err(error(key, Code::Busy));
        }
        if state.owner.is_some_and(|owner| owner != key.owner())
            || state.execution.is_some_and(|owner| owner != context.owner_id())
            || key.id().get() <= state.high_water
        {
            return Err(error(key, Code::InvalidIdentity));
        }
        let deadline = context.deadline().ok_or_else(|| error(key, Code::Admission))?;
        let now = context.now();
        if state.last_observed.is_some_and(|old| now < old) {
            state.closed = true;
            return Err(error(key, Code::Deadline));
        }
        if now >= deadline {
            return Err(error(key, Code::Deadline));
        }
        if deadline.as_nanos() - now.as_nanos() > DIAGNOSTIC_OPERATION_NANOS {
            return Err(error(key, Code::Admission));
        }
        if context.cancellation_requested() {
            return Err(error(key, Code::Cancelled));
        }
        state.owner = Some(key.owner());
        state.execution = Some(context.owner_id());
        state.last_observed = Some(now);
        state.operation = Some(Binding {
            key,
            execution: context.owner_id(),
            scope: context.scope_id(),
            deadline,
            cleanup: None,
            cleanup_scope: None,
            last_observed: now,
        });
        Ok(DiagnosticStorageOperation { shared: self.shared.clone(), key, deadline })
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
        if !state.stages.is_empty() || !state.reads.is_empty() || state.deletion.is_some() {
            return Err(error(operation.key, Code::Busy));
        }
        state.high_water = operation.key.id().get();
        state.operation = None;
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
        if namespace != operation.key.owner().namespace || state.namespace.is_some() {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        state
            .port
            .resolve(root, namespace, context)
            .map_err(|cause| native(operation.key, Code::ForeignProtection, cause))?;
        state.namespace = Some(namespace);
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
        if state.namespace.is_none() {
            return Err(error(operation.key, Code::Closed));
        }
        state
            .port
            .acquire_writer(context)
            .map_err(|cause| native(operation.key, Code::Busy, cause))?;
        state.writer = true;
        Ok(())
    }
    fn check(
        &self,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<MutexGuard<'_, State>, DiagnosticStorageError> {
        if !Arc::ptr_eq(&self.shared, &operation.shared) {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let mut state = lock(&self.shared);
        check(&mut state, operation.key, context, false)?;
        Ok(state)
    }
}
impl Drop for DiagnosticStorage {
    fn drop(&mut self) {
        if lock(&self.shared).port.live_handles() != 0 {
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
