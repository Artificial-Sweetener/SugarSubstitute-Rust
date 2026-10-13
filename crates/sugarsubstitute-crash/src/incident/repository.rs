// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the current protected operation and every unfinished native owner.
use super::{IncidentStoreError, StoredIncidentSummary, codec_ledger::digest};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_contracts::incident::*;
use sugarsubstitute_supervision::diagnostic_storage::*;

/// A repository bound to one actual sealed operation and its original deadline.
/// All methods run inside the execution owner's finite worker, never the UI thread.
pub struct IncidentRepository {
    pub(super) storage: DiagnosticStorage,
    pub(super) operation: DiagnosticStorageOperation,
    pub(super) reads: super::allocation::OwnerSlots<DiagnosticReadLease, DIAGNOSTIC_READ_BOUND>,
    pub(super) stages: super::allocation::OwnerSlots<DiagnosticStage, DIAGNOSTIC_STAGE_BOUND>,
    pub(super) pending: Option<super::publication::PendingPublication>,
    pub(super) preflight: Option<super::publication_preflight::RetainedPreflight>,
    pub(super) publication_identity: DiagnosticPublicationIdentity,
    pub(super) retention_utc: Option<IncidentUtc>,
    pub(super) next_ticket: u64,
}
impl IncidentRepository {
    /// Consume real protected storage and its separately admitted current operation.
    /// Historical persisted operation identities cannot establish this authority.
    /// The original context admits the retained publication identity before allocation.
    /// # Errors
    /// Rejects foreign/expired context, exhausted original allocation capacity or a
    /// retired operation. No replacement reservation or scheduling authority is created.
    pub fn new(
        mut storage: DiagnosticStorage,
        operation: DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<Self, IncidentStoreError> {
        let publication_identity = storage.recovery_publication_identity(&operation, context)?;
        Ok(Self {
            storage,
            operation,
            reads: super::allocation::OwnerSlots::new(),
            stages: super::allocation::OwnerSlots::new(),
            pending: None,
            preflight: None,
            publication_identity,
            retention_utc: None,
            next_ticket: 1,
        })
    }
    /// Return the current full matching identity without exposing native authority.
    pub const fn operation(&self) -> LogicalStoreOperationKey {
        self.operation.key()
    }
    /// Observe retained native custody and physical accounting independently of tickets.
    pub fn snapshot(&self) -> DiagnosticStorageSnapshot {
        self.storage.snapshot()
    }
    /// Checked-close actual owners while preserving every persisted object.
    /// # Errors
    /// An uncertain close retains the original storage operation and reservation.
    pub fn retire(&mut self, context: &JobContext) -> Result<(), IncidentStoreError> {
        self.storage.retire(&self.operation, context)?;
        self.reads.clear();
        self.stages.clear();
        self.pending = None;
        self.preflight = None;
        Ok(())
    }
    pub(super) fn namespace(&self, key: IncidentKey) -> Result<(), IncidentStoreError> {
        if key.subject().coordinator().namespace != self.operation.key().owner().namespace {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        Ok(())
    }
    pub(super) fn inventory(
        &mut self,
        context: &JobContext,
    ) -> Result<DiagnosticInventory, IncidentStoreError> {
        let inventory = self.storage.inventory(&self.operation, context)?;
        if inventory.status() != DiagnosticInventoryStatus::Complete {
            return Err(IncidentStoreError::Capacity);
        }
        Ok(inventory)
    }
    pub(super) fn read_candidate(
        &mut self,
        candidate: &DiagnosticInventoryEntry,
        context: &JobContext,
    ) -> Result<(DiagnosticRecoverySource, DiagnosticReadFacts, usize), IncidentStoreError> {
        if self.reads.len() >= DIAGNOSTIC_READ_BOUND || candidate.bytes() > 65_536 {
            return Err(IncidentStoreError::Capacity);
        }
        let bound = if candidate.class() == DiagnosticEntryClass::Seed { 4096 } else { 65_536 };
        let lease = self.storage.read_inventory(candidate, bound, &self.operation, context)?;
        let index = self.reads.len();
        self.reads.push(lease)?;
        let facts = self.reads.get_mut(index)?.facts();
        let bytes = self.reads.get_mut(index)?.whole_source(context)?;
        Ok((bytes, facts, index))
    }
    pub(super) fn close_read(
        &mut self,
        index: usize,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        self.reads.get_mut(index)?.retire(context)?;
        self.reads.remove(index)?;
        Ok(())
    }
    pub(super) fn activate_bytes(
        &mut self,
        object: DiagnosticObject,
        bytes: &[u8],
        expected: DiagnosticPredecessor,
        context: &JobContext,
    ) -> Result<DiagnosticActivation, IncidentStoreError> {
        if self.stages.len() >= DIAGNOSTIC_STAGE_BOUND {
            return Err(IncidentStoreError::Capacity);
        }
        self.inventory(context)?;
        let index = self.stages.len();
        self.stages.push(self.storage.stage(
            object,
            bytes.len() as u64,
            &self.operation,
            context,
        )?)?;
        self.stages.get_mut(index)?.write_chunk(bytes, context)?;
        let flushed = self.stages.get_mut(index)?.flush(context)?.read_facts();
        if flushed.sha256() != digest(bytes) || flushed.bytes() != bytes.len() as u64 {
            return Err(IncidentStoreError::WrongDigest);
        }
        let activation = self.storage.activate(
            self.stages.get_mut(index)?,
            expected,
            &self.operation,
            context,
        )?;
        self.stages.remove(index)?;
        Ok(activation)
    }
    pub(super) fn find_summary(
        &mut self,
        key: IncidentKey,
        context: &JobContext,
    ) -> Result<Option<(super::allocation::RetainedSummary, DiagnosticReadFacts)>, IncidentStoreError>
    {
        self.namespace(key)?;
        let inventory = self.inventory(context)?;
        let mut found = None;
        for candidate in
            inventory.entries().filter(|entry| entry.class() == DiagnosticEntryClass::Summary)
        {
            let (bytes, facts, index) = self.read_candidate(candidate, context)?;
            let custody = self.storage.recovery_record(&self.operation, context)?;
            let mut decode = self.storage.recovery_decode(&self.operation, context)?;
            let summary =
                super::allocation::decode(&mut decode, || super::decode_summary(bytes.as_bytes()))?;
            let summary = super::allocation::RetainedSummary::new(summary, custody);
            self.reads
                .get_mut(index)?
                .bind_object(DiagnosticObject::Summary(summary.summary().seed().key()), context)?;
            if summary.summary().seed().key() == key {
                if found.is_some() {
                    return Err(IncidentStoreError::RecoveryConflict);
                }
                found = Some((summary, facts));
            }
            self.close_read(index, context)?;
        }
        Ok(found)
    }
    pub(super) fn ledger(
        &mut self,
        context: &JobContext,
    ) -> Result<SummaryEvictions, IncidentStoreError> {
        let inventory = self.inventory(context)?;
        let mut ledger = None;
        for candidate in
            inventory.entries().filter(|entry| entry.class() == DiagnosticEntryClass::StoreState)
        {
            if ledger.is_some() {
                return Err(IncidentStoreError::RecoveryConflict);
            }
            let (bytes, _, index) = self.read_candidate(candidate, context)?;
            self.reads.get_mut(index)?.bind_object(DiagnosticObject::StoreState, context)?;
            let mut decode = self.storage.recovery_decode(&self.operation, context)?;
            let super::IncidentStoreState::Ledger(value) =
                super::allocation::decode(&mut decode, || {
                    super::decode_store_state(bytes.as_bytes())
                })?
            else {
                return Err(IncidentStoreError::RecoveryConflict);
            };
            if value.namespace() != self.operation.key().owner().namespace {
                return Err(IncidentStoreError::RecoveryConflict);
            }
            self.close_read(index, context)?;
            ledger = Some(value);
        }
        ledger.ok_or(IncidentStoreError::RecoveryConflict)
    }
    pub(super) fn verify_payloads(
        &mut self,
        stored: &StoredIncidentSummary,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        let summary = stored.summary();
        let PayloadDisposition::Retained(inventory) = summary.facts().payload() else {
            return Ok(());
        };
        let revision = stored.payload_revision().ok_or(IncidentStoreError::MissingPayload)?;
        for fact in inventory.entries() {
            if self.reads.len() >= DIAGNOSTIC_READ_BOUND {
                return Err(IncidentStoreError::Capacity);
            }
            let lease = self.storage.read(
                DiagnosticObject::Payload(summary.seed().key(), revision, fact.kind()),
                fact.retained_bytes(),
                &self.operation,
                context,
            )?;
            let index = self.reads.len();
            self.reads.push(lease)?;
            let actual = self.reads.get_mut(index)?.facts();
            if actual.bytes() != fact.retained_bytes() || actual.sha256() != *fact.sha256() {
                return Err(IncidentStoreError::WrongDigest);
            }
            self.close_read(index, context)?;
        }
        Ok(())
    }
}

pub(super) fn predecessor(facts: DiagnosticReadFacts) -> DiagnosticPredecessor {
    DiagnosticPredecessor::Exact { file_identity: facts.file_identity(), sha256: facts.sha256() }
}
