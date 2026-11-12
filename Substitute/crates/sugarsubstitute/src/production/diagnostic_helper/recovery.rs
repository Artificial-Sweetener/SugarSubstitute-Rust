// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover canonical pending data on finite workers without reconstructing old process ownership.
#[path = "recovery/service.rs"]
mod service;
#[path = "recovery/source.rs"]
mod source;
pub(in crate::production) use service::{RecoveryIssue, RecoveryService};
use sugarsubstitute_application::incident::IncidentRecoveryScope;
use sugarsubstitute_contracts::{execution::*, incident::*};
use sugarsubstitute_crash::incident::{
    HistoricalCaptureRecovery, HistoricalCaptureRecoveryState, IncidentRepository,
    IncidentStoreError, PendingIncidentSnapshot, SummaryMigrationWork,
};
use sugarsubstitute_supervision::diagnostic_storage::*;
/// Fixed source metadata within the existing controller reservation; scan data uses its8MiB job.
pub(in crate::production) const RECOVERY_SOURCE_BYTES: u64 = 16 * 1024;
/// Fixed canonical Journal-name cursor; independent of native enumeration order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) enum RecoveryPhase {
    Migrate,
    Capture { after: Option<[u8; 32]> },
    Producer { journal: [u8; 32] },
    Scan,
}
pub(in crate::production) struct RecoveryInput {
    pub(in crate::production) request: ProtectedDiagnosticWriteRequest,
    pub(in crate::production) scope: IncidentRecoveryScope,
    pub(in crate::production) at: IncidentUtc,
    pub(in crate::production) capture: Option<sugarsubstitute_crash::CaptureRootSource>,
    pub(in crate::production) storage_operation: LogicalStoreOperationKey,
    pub(in crate::production) phase: RecoveryPhase,
}
pub(in crate::production) enum RecoveryCause {
    Store(IncidentStoreError),
    Memory(MemoryClaimError),
}
impl std::fmt::Debug for RecoveryCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RetainedRecoveryCause")
    }
}
impl std::fmt::Display for RecoveryCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Pending incident recovery failed")
    }
}
impl std::error::Error for RecoveryCause {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Store(cause) => cause,
            Self::Memory(cause) => cause,
        })
    }
}
pub(in crate::production) struct RecoveryFailure {
    cause: RecoveryCause,
    retirement: Option<IncidentStoreError>,
    _original: MemoryLease,
}
impl RecoveryFailure {
    /// Borrow independent checked-close failure without replacing the original cause.
    /// Both owned errors deallocate before their actual original Main reservation returns.
    pub(in crate::production) fn retirement_failure(&self) -> Option<&IncidentStoreError> {
        self.retirement.as_ref()
    }
}
impl std::fmt::Debug for RecoveryFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainedRecoveryFailure")
            .field("retirement_failure_retained", &self.retirement_failure().is_some())
            .finish_non_exhaustive()
    }
}
impl std::fmt::Display for RecoveryFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Pending incident recovery failed")
    }
}
impl std::error::Error for RecoveryFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
struct RecoveryOperation {
    input: Option<RecoveryInput>,
    repository: Option<IncidentRepository>,
    pending: Option<PendingIncidentSnapshot>,
    migration: Option<SummaryMigrationWork>,
    historical: Option<HistoricalCaptureRecovery>,
    origin: Option<CheckedRecoveryCaptureOrigin>,
    next: Option<RecoveryPhase>,
    historical_state: Option<HistoricalCaptureRecoveryState>,
    native_source_unavailable: bool,
    promotion_replayed: bool,
    first: Option<IncidentStoreError>,
    memory: Option<MemoryClaimError>,
    retirement: Option<IncidentStoreError>,
    retired: bool,
    cause_transferred: bool,
    // Actual Main context, retained after native/data fields through final Drop.
    original: Option<MemoryLease>,
}
impl RecoveryOperation {
    fn new() -> Self {
        Self {
            input: None,
            repository: None,
            pending: None,
            migration: None,
            historical: None,
            origin: None,
            next: None,
            historical_state: None,
            native_source_unavailable: false,
            promotion_replayed: false,
            first: None,
            memory: None,
            retirement: None,
            retired: false,
            cause_transferred: false,
            original: None,
        }
    }
    fn execute(&mut self, input: RecoveryInput, context: &JobContext) {
        self.original = Some(context.memory().clone());
        self.input = Some(input);
        let claim = match context.memory().claim(DIAGNOSTIC_STORAGE_BYTES) {
            Ok(claim) => claim,
            Err(cause) => {
                self.memory = Some(cause);
                return;
            }
        };
        let result = (|| -> Result<(), IncidentStoreError> {
            let input = self.input.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?;
            if input.storage_operation.owner() != input.scope.operation().owner()
                || (input.phase == RecoveryPhase::Scan)
                    != (input.storage_operation == input.scope.operation())
            {
                return Err(IncidentStoreError::RecoveryConflict);
            }
            let mut storage = DiagnosticStorage::new(claim)?;
            let operation = storage.begin_operation(input.storage_operation, context)?;
            self.repository = Some(IncidentRepository::new(storage, operation, context)?);
            let repository =
                self.repository.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
            let opening = repository.resolve_namespace(&input.request, context)?;
            repository.acquire_writer(context)?;
            match opening {
                DiagnosticStoreOpening::Created(mut genesis) => {
                    repository.initialize(&mut genesis, context)?;
                }
                DiagnosticStoreOpening::Existing => {
                    repository.validate_store(context)?;
                }
            }
            match input.phase {
                RecoveryPhase::Migrate => {
                    let historical =
                        self.historical.as_mut().ok_or(IncidentStoreError::Capacity)?;
                    if repository.recover_one_capture_promotion(
                        input.capture.as_ref(),
                        historical,
                        context,
                    )? {
                        self.promotion_replayed = true;
                        self.next = Some(RecoveryPhase::Migrate);
                        return Ok(());
                    }
                    let changed =
                        repository.migrate_one_pending_summary(&mut self.migration, context)?;
                    self.next = Some(if changed {
                        RecoveryPhase::Migrate
                    } else if input.capture.is_some() {
                        RecoveryPhase::Capture { after: None }
                    } else {
                        self.native_source_unavailable = true;
                        RecoveryPhase::Scan
                    });
                }
                RecoveryPhase::Scan => {
                    if input.storage_operation != input.scope.operation() {
                        return Err(IncidentStoreError::RecoveryConflict);
                    }
                    self.pending = Some(repository.pending_incidents(input.at, context)?);
                }
                phase => self.recover_capture(phase, context)?,
            }
            Ok(())
        })();
        if let Err(cause) = result {
            self.first = Some(cause);
        }
        tracing::info!(operation=?self.input.as_ref().map(|input|input.storage_operation),
            phase=?self.input.as_ref().map(|input|input.phase),
            native_source_unavailable=self.native_source_unavailable,
            promotion_replayed=self.promotion_replayed,
            historical_state=?self.historical_state,
            frame_failure=?self.historical.as_ref().and_then(HistoricalCaptureRecovery::frame_failure),
            native_failure=?self.historical.as_ref().and_then(HistoricalCaptureRecovery::report).and_then(|report|report.failure_diagnostic()),
            journal_native_failure=?self.historical.as_ref().and_then(HistoricalCaptureRecovery::report).and_then(|report|report.journal_failure_diagnostic()),
            journal_storage_failure=?self.historical.as_ref().and_then(HistoricalCaptureRecovery::report).and_then(|report|report.journal_storage_failure()),
            journal_post_panicked=self.historical.as_ref().and_then(HistoricalCaptureRecovery::report).is_some_and(|report|report.journal_post_panicked()),
            failed=self.first.is_some()||self.memory.is_some(),
            "Incident recovery phase observed");
    }
    fn retire_capture(
        &mut self,
        client: &sugarsubstitute_execution::ExecutionClient,
        context: &JobContext,
    ) -> bool {
        let result = (|| -> Result<(), IncidentStoreError> {
            if let Some(owner) = &mut self.historical {
                owner.retire(client, context)?;
            }
            if let Some(origin) = &mut self.origin {
                origin.retire(context)?;
            }
            Ok(())
        })();
        if let Err(cause) = result {
            if self.retirement.is_none() {
                self.retirement = Some(cause);
            }
            return false;
        }
        self.retire(context)
    }
    fn retire(&mut self, context: &JobContext) -> bool {
        if self.retired {
            return true;
        }
        let result = match (&mut self.repository, &mut self.pending) {
            (Some(repository), Some(pending)) => repository.retire_pending(pending, context),
            (Some(repository), None) => repository.retire(context),
            (None, _) => Ok(()),
        };
        match result {
            Ok(()) => {
                self.retired = true;
                true
            }
            Err(cause) => {
                if self.retirement.is_none() {
                    self.retirement = Some(cause);
                }
                false
            }
        }
    }
    fn failed(&self) -> bool {
        self.cause_transferred
            || self.first.is_some()
            || self.memory.is_some()
            || self.retirement.is_some()
    }
}

#[cfg(test)]
#[path = "recovery/tests.rs"]
mod tests;

#[path = "recovery/historical.rs"]
mod historical;
