// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own account-derived protected claim storage and pinned filesystem identities.
//! Native handles are noninheritable. Finite workers retain this state before FFI;
//! checked closure preserves failed handles and active records for explicit retry.
mod account;
pub(crate) mod backend_custody;
mod backend_file;
mod backend_recovery;
pub(crate) mod files;
mod paths;
mod records;
use crate::protected_storage::exclusion::HeldFile;
use crate::{ClaimError, claims::pool::SlotUse};
use files::Handle;
#[cfg(test)]
pub(crate) use records::{
    CoordinatorPredecessor, permits_replacement, prepare_backend_release,
    prepare_coordinator_release,
};
pub(crate) use records::{Record, RecordState};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::{
    configuration::LaunchConfiguration,
    lifecycle::{CoordinatorKey, ResourceKey},
};

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
pub(crate) struct Root {
    pub(crate) key: Option<CoordinatorKey>,
    pub(crate) sid: Vec<u8>,
    pub(crate) directory: PathBuf,
    pub(crate) coordinator: Option<HeldFile>,
    pub(crate) record: Option<Record>,
    pub(crate) original_released: Option<[u8; 128]>,
    pub(crate) handles: Vec<Handle>,
    pub(crate) pool: Option<Arc<crate::claims::pool::Pool>>,
    pub(crate) claimed: bool,
    pub(crate) execution_owner: Option<sugarsubstitute_contracts::execution::ExecutionId>,
    pub(crate) close_deadline: Option<sugarsubstitute_contracts::execution::Moment>,
    pub(crate) entry_election: Option<crate::coordinator_entry::election::RetainedElectionProof>,
}
impl Root {
    pub(crate) fn new() -> Self {
        Self {
            key: None,
            sid: Vec::new(),
            directory: PathBuf::new(),
            coordinator: None,
            record: None,
            original_released: None,
            handles: Vec::new(),
            pool: None,
            claimed: false,
            execution_owner: None,
            close_deadline: None,
            entry_election: None,
        }
    }
}
pub(crate) struct Resolution {
    pub(crate) configuration: LaunchConfiguration,
    pub(crate) digest: [u8; 32],
    pub(crate) account_sid: Vec<u8>,
    pub(crate) ready: bool,
    pub(crate) handles: Mutex<Vec<Handle>>,
    pub(crate) slot: Arc<SlotUse>,
}
impl Resolution {
    pub(crate) fn new(configuration: LaunchConfiguration, slot: Arc<SlotUse>) -> Self {
        Self {
            configuration,
            digest: [0; 32],
            account_sid: Vec::new(),
            ready: false,
            handles: Mutex::new(Vec::new()),
            slot,
        }
    }
    pub(crate) fn close(&self) -> Result<(), ClaimError> {
        files::close_all(&mut lock(&self.handles))
    }
}
pub(crate) struct Backend {
    pub(crate) custodian: Option<crate::BackendCustodyParent>,
    pub(crate) resource: ResourceKey,
    pub(crate) proof: Arc<Resolution>,
    pub(crate) file: Option<HeldFile>,
    pub(crate) custody_probe: Option<HeldFile>,
    pub(crate) record: Option<Record>,
    pub(crate) predecessor: Option<[u8; 128]>,
    pub(crate) claimed: bool,
    pub(crate) released: bool,
}

/// Real adapter boundary used by owner tests; it grants no scheduling capability.
pub(crate) trait NativePort: Send + Sync {
    fn initialize(
        &self,
        root: &mut Root,
        installation: &Path,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError>;
    fn resolve(&self, resolution: &mut Resolution, sid: &[u8]) -> Result<(), ClaimError>;
    fn acquire(
        &self,
        backend: &mut Backend,
        directory: &Path,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError>;
    fn publish(
        &self,
        backend: &mut Backend,
        state: RecordState,
        process: u64,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError>;
    fn release(
        &self,
        backend: &mut Backend,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError>;
    fn close(
        &self,
        root: &mut Root,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError>;
}
pub(crate) struct SystemPort;
impl NativePort for SystemPort {
    fn initialize(
        &self,
        root: &mut Root,
        installation: &Path,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        account::initialize(root, installation, context)
    }
    fn resolve(&self, resolution: &mut Resolution, sid: &[u8]) -> Result<(), ClaimError> {
        paths::resolve(resolution, sid)
    }
    fn acquire(
        &self,
        backend: &mut Backend,
        directory: &Path,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        backend_file::acquire_backend(backend, directory, context)?;
        self.publish(backend, RecordState::LaunchPending, 0, context)
    }
    fn publish(
        &self,
        backend: &mut Backend,
        state: RecordState,
        process: u64,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        let record =
            backend.record.as_mut().ok_or_else(|| ClaimError::invalid("missing_record"))?;
        record.state = state;
        record.process = process;
        records::publish(
            backend.file.as_ref().ok_or_else(|| ClaimError::invalid("missing_claim"))?,
            record,
            context,
        )
    }
    fn release(
        &self,
        backend: &mut Backend,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        check_close_context(context)?;
        backend.proof.close()?;
        check_close_context(context)?;
        if let Some(file) = backend.file.as_ref().filter(|file| file.is_locked()) {
            records::retire_backend(
                file,
                &mut backend.record,
                backend.predecessor.as_ref(),
                context,
            )?;
        }
        check_close_context(context)?;
        if let Some(probe) = &mut backend.custody_probe {
            probe.close()?;
        }
        backend.custody_probe = None;
        if let Some(file) = &mut backend.file {
            file.close()?;
        }
        backend.file = None;
        backend.claimed = false;
        backend.released = true;
        Ok(())
    }
    fn close(
        &self,
        root: &mut Root,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        if root.execution_owner.is_some_and(|owner| owner != context.owner_id()) {
            return Err(ClaimError::invalid("foreign_claim_close_execution"));
        }
        let deadline =
            context.deadline().ok_or_else(|| ClaimError::invalid("claim_close_deadline"))?;
        if root.close_deadline.is_some_and(|old| deadline > old) {
            return Err(ClaimError::invalid("claim_close_renewed"));
        }
        root.close_deadline = Some(root.close_deadline.map_or(deadline, |old| old.min(deadline)));
        if context.cancellation_requested() || context.now() >= deadline {
            return Err(ClaimError::invalid("claim_close_expired"));
        }
        if let Some(file) = root.coordinator.as_ref().filter(|file| file.is_locked()) {
            let released = match root.entry_election.as_ref().map(|proof| &proof.origin) {
                Some(crate::coordinator_entry::election::ElectionRecordOrigin::Released(bytes)) => {
                    Some(records::CoordinatorPredecessor::Released(bytes))
                }
                Some(crate::coordinator_entry::election::ElectionRecordOrigin::RecoveredHeld {
                    record,
                    ..
                }) => Some(records::CoordinatorPredecessor::RecoveredHeld(record)),
                _ => root.original_released.as_ref().map(records::CoordinatorPredecessor::Released),
            };
            records::retire_coordinator(file, &mut root.record, released, context)?;
        }
        check_close_context(context)?;
        for handle in root.handles.iter_mut().rev() {
            check_close_context(context)?;
            handle.close()?;
        }
        root.handles.clear();
        check_close_context(context)?;
        if let Some(file) = &mut root.coordinator {
            file.close()?;
        }
        check_close_context(context)?;
        root.coordinator = None;
        root.record = None;
        root.original_released = None;
        root.entry_election = None;
        root.pool = None;
        Ok(())
    }
}

fn check_close_context(
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<(), ClaimError> {
    if context.cancellation_requested()
        || context.deadline().is_none_or(|until| context.now() >= until)
    {
        Err(ClaimError::invalid("claim_close_expired"))
    } else {
        Ok(())
    }
}
