// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move the original passive logical failure without releasing its source or completion.
use super::*;
use std::{error::Error, fmt};

/// Identify the actual last entered operation, independently of worker termination.
#[derive(Clone, Copy, Debug)]
pub(super) enum SeedStage {
    NotStarted,
    Memory,
    Storage,
    Operation,
    Repository,
    Namespace,
    Writer,
    Initialize,
    Validate,
    Publish,
    Committed,
}
/// Preserve the original logical error and independent checked-cleanup error once.
pub(in crate::production) struct SeedFailure {
    stage: SeedStage,
    store: Option<IncidentStoreError>,
    memory: Option<MemoryClaimError>,
    cleanup: Option<IncidentStoreError>,
}
impl SeedFailure {
    /// Transfer only passive cause values; repository, opening, input and job custody stay put.
    pub(super) fn take(operation: &mut SeedOperation) -> Option<Self> {
        if operation.first.is_none()
            && operation.memory.is_none()
            && operation.retirement.is_none()
            && operation.committed.is_some()
        {
            return None;
        }
        Some(Self {
            stage: operation.stage,
            store: operation.first.take(),
            memory: operation.memory.take(),
            cleanup: operation.retirement.take(),
        })
    }
    /// Render only closed categories and concrete bounded native metadata, never arbitrary causes.
    pub(super) fn record(
        &self,
        completion: &sugarsubstitute_contracts::execution::JobCompletion<
            (),
            super::SeedServiceIssue,
        >,
    ) {
        let storage = self.store.as_ref().or(self.cleanup.as_ref()).and_then(|cause| {
            if let IncidentStoreError::Storage(storage) = cause { Some(storage) } else { None }
        });
        let native = storage
            .and_then(Error::source)
            .and_then(|cause| cause.downcast_ref::<sugarsubstitute_supervision::ClaimError>());
        tracing::error!(
            operation = "incident_seed_transaction_failed",
            scope = ?completion.scope_id, job = ?completion.spec.id,
            worker_terminal = ?completion.outcome.kind(), stage = ?self.stage,
            store_code = ?self.store.as_ref().map(IncidentStoreError::code),
            storage_code = ?storage.map(sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorageError::code),
            native_stage = native.map(sugarsubstitute_supervision::ClaimError::operation),
            native_win32_code = native.and_then(sugarsubstitute_supervision::ClaimError::win32_code),
            memory_requested_bytes = self.memory.map(|cause| cause.requested_bytes),
            memory_available_bytes = self.memory.map(|cause| cause.available_bytes),
            cleanup_code = ?self.cleanup.as_ref().map(IncidentStoreError::code),
            missing_commit = self.store.is_none() && self.memory.is_none() && self.cleanup.is_none(),
            queue_time_ns = %completion.queue_time.as_nanos(),
            execution_time_ns = %completion.execution_time.as_nanos(),
            deadline_ns = completion.spec.deadline.map(sugarsubstitute_contracts::execution::Moment::as_nanos),
            "Incident seed transaction failed with its original cause retained"
        );
    }
}
impl fmt::Debug for SeedFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SeedFailure")
            .field("stage", &self.stage)
            .field("store_code", &self.store.as_ref().map(IncidentStoreError::code))
            .field("memory", &self.memory)
            .field("cleanup_code", &self.cleanup.as_ref().map(IncidentStoreError::code))
            .finish()
    }
}
impl fmt::Display for SeedFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Incident seed transaction failed at {:?}", self.stage)
    }
}
impl Error for SeedFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Some(cause) = &self.store {
            Some(cause)
        } else if let Some(cause) = &self.memory {
            Some(cause)
        } else {
            self.cleanup.as_ref().map(|cause| cause as &dyn Error)
        }
    }
}
