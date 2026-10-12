// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own account-derived protected claim storage and pinned filesystem identities.
//! Native handles are noninheritable. Finite workers retain this state before FFI;
//! checked closure preserves failed handles and active records for explicit retry.
mod account;
pub(crate) mod files;
mod paths;
mod records;
use crate::protected_storage::security;
use crate::{ClaimError, claims::pool::SlotUse};
use files::{Handle, HeldFile};
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
    pub(crate) handles: Vec<Handle>,
    pub(crate) pool: Option<Arc<crate::claims::pool::Pool>>,
    pub(crate) claimed: bool,
}
impl Root {
    pub(crate) fn new() -> Self {
        Self {
            key: None,
            sid: Vec::new(),
            directory: PathBuf::new(),
            coordinator: None,
            record: None,
            handles: Vec::new(),
            pool: None,
            claimed: false,
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
    pub(crate) resource: ResourceKey,
    pub(crate) proof: Arc<Resolution>,
    pub(crate) file: Option<HeldFile>,
    pub(crate) record: Option<Record>,
    pub(crate) claimed: bool,
    pub(crate) released: bool,
}

/// Real adapter boundary used by owner tests; it grants no scheduling capability.
pub(crate) trait NativePort: Send + Sync {
    fn initialize(&self, root: &mut Root, installation: &Path) -> Result<(), ClaimError>;
    fn resolve(&self, resolution: &mut Resolution, sid: &[u8]) -> Result<(), ClaimError>;
    fn acquire(&self, backend: &mut Backend, directory: &Path) -> Result<(), ClaimError>;
    fn publish(
        &self,
        backend: &mut Backend,
        state: RecordState,
        process: u64,
    ) -> Result<(), ClaimError>;
    fn release(&self, backend: &mut Backend) -> Result<(), ClaimError>;
    fn close(&self, root: &mut Root) -> Result<(), ClaimError>;
}
pub(crate) struct SystemPort;
impl NativePort for SystemPort {
    fn initialize(&self, root: &mut Root, installation: &Path) -> Result<(), ClaimError> {
        account::initialize(root, installation)
    }
    fn resolve(&self, resolution: &mut Resolution, sid: &[u8]) -> Result<(), ClaimError> {
        paths::resolve(resolution, sid)
    }
    fn acquire(&self, backend: &mut Backend, directory: &Path) -> Result<(), ClaimError> {
        files::acquire_backend(backend, directory)?;
        self.publish(backend, RecordState::LaunchPending, 0)
    }
    fn publish(
        &self,
        backend: &mut Backend,
        state: RecordState,
        process: u64,
    ) -> Result<(), ClaimError> {
        let record =
            backend.record.as_mut().ok_or_else(|| ClaimError::invalid("missing_record"))?;
        record.state = state;
        record.process = process;
        records::publish(
            backend.file.as_ref().ok_or_else(|| ClaimError::invalid("missing_claim"))?,
            record,
        )
    }
    fn release(&self, backend: &mut Backend) -> Result<(), ClaimError> {
        backend.proof.close()?;
        if backend.file.as_ref().is_some_and(HeldFile::is_locked)
            && let Some(record) = &mut backend.record
        {
            record.state = RecordState::Released;
            record.process = 0;
            records::publish(
                backend.file.as_ref().ok_or_else(|| ClaimError::invalid("missing_claim"))?,
                record,
            )?;
        }
        if let Some(file) = &mut backend.file {
            file.close()?;
        }
        backend.file = None;
        backend.claimed = false;
        backend.released = true;
        Ok(())
    }
    fn close(&self, root: &mut Root) -> Result<(), ClaimError> {
        if root.coordinator.as_ref().is_some_and(HeldFile::is_locked)
            && let Some(record) = &mut root.record
        {
            record.state = RecordState::Released;
            records::publish(
                root.coordinator.as_ref().ok_or_else(|| ClaimError::invalid("missing_claim"))?,
                record,
            )?;
        }
        files::close_all(&mut root.handles)?;
        if let Some(file) = &mut root.coordinator {
            file.close()?;
        }
        root.coordinator = None;
        root.pool = None;
        Ok(())
    }
}
