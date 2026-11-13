// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read canonical pending summaries through actual bounded protected inventory and checked closure.
use super::{
    IncidentRepository, IncidentStoreError, StoredIncidentSummary, allocation::SummaryCollection,
};
use sugarsubstitute_contracts::{execution::JobContext, incident::*};
/// Actual bounded stored data, retained with its original collection allocation.
/// This does not contain an original publication operation because Summary2 persists none.
pub struct PendingIncidentSnapshot {
    operation: LogicalStoreOperationKey,
    records: SummaryCollection,
    retired: bool,
}
impl PendingIncidentSnapshot {
    /// Borrow historical data only after this snapshot's actual readers/writer/root retired.
    /// # Errors
    /// Returns RecoveryConflict until independently checked repository retirement succeeds.
    pub fn summaries(
        &self,
    ) -> Result<impl Iterator<Item = &StoredIncidentSummary>, IncidentStoreError> {
        if !self.retired {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        Ok(self
            .records
            .iter()
            .map(|(stored, _)| stored)
            .filter(|stored| stored.summary().facts().retention().acknowledged().is_none()))
    }
    /// Current scan operation is correlation, never historical publication/native ownership.
    pub const fn operation(&self) -> LogicalStoreOperationKey {
        self.operation
    }
}
impl IncidentRepository {
    /// Scan the existing bounded canonical inventory without changing persisted records.
    /// Uses the storage owner's literal128 collection admission, whole-source validation,
    /// namespace/object binding, duplicate/time checks and actual attachment inventory.
    /// # Errors
    /// Unknown/corrupt/partial/future records or uncertain reads preserve original custody.
    /// No historical operation, dump, capture completion or family settlement is synthesized.
    pub fn pending_incidents(
        &mut self,
        now: IncidentUtc,
        context: &JobContext,
    ) -> Result<PendingIncidentSnapshot, IncidentStoreError> {
        self.validate_store(context)?;
        let (_inventory, records) = self.publication_inventory(now, context)?;
        Ok(PendingIncidentSnapshot { operation: self.operation(), records, retired: false })
    }
    /// Checked-close this same current scan before exposing any historical projection.
    /// # Errors
    /// Foreign operation or uncertain native closure retains snapshot and repository unchanged.
    pub fn retire_pending(
        &mut self,
        pending: &mut PendingIncidentSnapshot,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        if pending.operation != self.operation() {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        if pending.retired {
            return Ok(());
        }
        self.retire(context)?;
        pending.retired = true;
        Ok(())
    }
}
