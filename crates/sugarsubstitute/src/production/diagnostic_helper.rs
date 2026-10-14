// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run a finite seed operation while retaining every partial native owner and first cause.
#[path = "diagnostic_helper/exporting.rs"]
pub(in crate::production) mod exporting;
use sugarsubstitute_contracts::{
    execution::{JobContext, MemoryClaimError},
    incident::{IncidentSeed, LogicalStoreOperationKey},
};
use sugarsubstitute_crash::incident::{IncidentRepository, IncidentStoreError, SeedCommit};
use sugarsubstitute_supervision::diagnostic_storage::{
    DIAGNOSTIC_STORAGE_BYTES, DiagnosticStorage, DiagnosticStoreOpening,
    ProtectedDiagnosticWriteRequest,
};

/// Borrow no path or reconstructed namespace privilege from a diagnostic command.
pub(in crate::production) struct SeedInput {
    pub(in crate::production) request: ProtectedDiagnosticWriteRequest,
    pub(in crate::production) operation: LogicalStoreOperationKey,
    pub(in crate::production) seed: IncidentSeed,
}
/// Retain the same original repository, opening and proofs across success or failure.
/// The parent observes terminal transfer and actual worker settlement separately.
pub(in crate::production) struct SeedOperation {
    repository: Option<IncidentRepository>,
    opening: Option<DiagnosticStoreOpening>,
    committed: Option<SeedCommit>,
    first: Option<IncidentStoreError>,
    retirement: Option<IncidentStoreError>,
    memory: Option<MemoryClaimError>,
    retired: bool,
}
impl SeedOperation {
    /// Construct inert retained custody before invoking any external provider.
    fn new() -> Self {
        Self {
            repository: None,
            opening: None,
            committed: None,
            first: None,
            retirement: None,
            memory: None,
            retired: false,
        }
    }
    /// Execute only inside the actual finite8MiB execution worker.
    /// The caller's owned source reservation retains the input until this call;
    /// repository record roles admit each moved seed destination before publication.
    fn execute(&mut self, input: SeedInput, context: &JobContext) {
        let memory = match context.memory().claim(DIAGNOSTIC_STORAGE_BYTES) {
            Ok(memory) => memory,
            Err(cause) => {
                self.memory = Some(cause);
                return;
            }
        };
        let prepare = (|| {
            // Admit the repository identity before protected filesystem effects.
            let mut storage = DiagnosticStorage::new(memory)?;
            let operation = storage.begin_operation(input.operation, context)?;
            self.repository = Some(IncidentRepository::new(storage, operation, context)?);
            let repository =
                self.repository.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
            self.opening = Some(repository.resolve_namespace(&input.request, context)?);
            repository.acquire_writer(context)?;
            self.publish(input.seed, context)
        })();
        if let Err(cause) = prepare {
            self.first = Some(cause);
        }
    }
    fn publish(
        &mut self,
        seed: IncidentSeed,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        let repository = self.repository.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        match self.opening.as_mut().ok_or(IncidentStoreError::RecoveryConflict)? {
            DiagnosticStoreOpening::Created(genesis) => {
                repository.initialize(genesis, context)?;
            }
            DiagnosticStoreOpening::Existing => {
                repository.validate_store(context)?;
            }
        }
        self.committed = Some(repository.publish_seed(seed, context)?);
        Ok(())
    }
    /// Borrow the genuine seed activation after strict ledger and seed publication.
    pub(in crate::production) fn committed(&self) -> Option<&SeedCommit> {
        self.committed.as_ref()
    }
    /// Borrow the unchanged original storage/publication cause.
    pub(in crate::production) fn failure(&self) -> Option<&IncidentStoreError> {
        self.first.as_ref()
    }
    /// Borrow a genuine original admission failure without allocating a replacement cause.
    pub(in crate::production) fn memory_failure(&self) -> Option<&MemoryClaimError> {
        self.memory.as_ref()
    }
    /// Borrow the independently retained failed checked cleanup cause.
    pub(in crate::production) fn retirement_failure(&self) -> Option<&IncidentStoreError> {
        self.retirement.as_ref()
    }
    /// Checked-close under the actual operation's accepted original cleanup context.
    /// Failure leaves the same repository and opening retained; no deadline is renewed.
    pub(in crate::production) fn retire(&mut self, context: &JobContext) -> bool {
        if self.retired {
            return true;
        }
        let Some(repository) = &mut self.repository else {
            // Construction preceding namespace resolution has no native file effects.
            self.retired = true;
            return true;
        };
        match repository.retire(context) {
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
    /// Observe checked file retirement; this does not claim worker or source-charge release.
    pub(in crate::production) const fn files_retired(&self) -> bool {
        self.retired
    }
}

#[path = "diagnostic_helper/service.rs"]
mod service;
#[path = "diagnostic_helper/source.rs"]
mod source;
pub(in crate::production) use service::{SeedService, SeedServiceIssue};
pub(in crate::production) use source::SEED_SOURCE_BYTES;

#[cfg(test)]
#[path = "diagnostic_helper/tests.rs"]
mod tests;

#[path = "diagnostic_helper/collection.rs"]
pub(in crate::production) mod collection;

#[path = "diagnostic_helper/reporting.rs"]
pub(in crate::production) mod reporting;
