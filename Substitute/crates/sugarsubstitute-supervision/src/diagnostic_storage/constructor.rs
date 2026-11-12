// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit exact owner metadata and every fixed destination before external storage effects.
use super::*;
impl DiagnosticStorage {
    /// Retain the complete8MiB claim before any filesystem effect.
    /// # Errors
    /// Rejects insufficient claim or bounded owner-state allocation failure.
    pub fn new(memory: MemoryClaim) -> Result<Self, DiagnosticStorageError> {
        if memory.bytes() < DIAGNOSTIC_STORAGE_BYTES {
            return Err(DiagnosticStorageError::rejected(None, Code::Capacity));
        }
        let allocation = allocation::owner(memory)?;
        let port = crate::durable_storage::protected::record_file::StreamingPort::admit(
            allocation.native_grant(),
        )
        .map_err(|cause| DiagnosticStorageError::native(None, Code::Admission, cause))?;
        Self::from_port(allocation, port)
    }
    pub(super) fn from_port(
        allocation: allocation::DiagnosticAllocationOwner,
        port: OwnedPort,
    ) -> Result<Self, DiagnosticStorageError> {
        let reservation = allocation::reservation(&allocation, None)?;
        let mut stages = Vec::new();
        let mut reads = Vec::new();
        let mut archives = Vec::new();
        let mut origins = Vec::new();
        stages
            .try_reserve_exact(DIAGNOSTIC_STAGE_BOUND)
            .and_then(|()| reads.try_reserve_exact(DIAGNOSTIC_READ_BOUND))
            .and_then(|()| archives.try_reserve_exact(DIAGNOSTIC_MIGRATION_ARCHIVE_BOUND))
            .and_then(|()| origins.try_reserve_exact(capture_read::origin::ORIGIN_BOUND))
            .map_err(|cause| reservation.failed(cause))?;
        if stages.capacity() != DIAGNOSTIC_STAGE_BOUND
            || reads.capacity() != DIAGNOSTIC_READ_BOUND
            || archives.capacity() != DIAGNOSTIC_MIGRATION_ARCHIVE_BOUND
            || origins.capacity() != capture_read::origin::ORIGIN_BOUND
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
                    capture_handles: 0,
                    finished_owner: None,
                    capture: None,
                    deletion: None,
                    migration: None,
                    payload: None,
                    archives,
                    origins,
                    scan_generation: 0,
                    scan_entries: Vec::new(),
                    next_lease: 1,
                    _scan_allocation: None,
                    allocation,
                })),
            },
        })
    }
}
