// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Perform one contiguous migration per genuinely admitted operation with durable partial custody.
mod reconciliation;
use super::{
    IncidentFileFacts, IncidentRepository, IncidentStoreError, IncidentStoreState,
    IncidentSummaryMigrationValidator, SummaryMigrationArchive, SummaryMigrationJournal,
    codec_ledger::digest,
};
use std::io::Write;
use sugarsubstitute_contracts::{execution::JobContext, incident::*};
use sugarsubstitute_supervision::diagnostic_storage::*;
/// Keep one operation's actual partial migration owners until checked repository retirement.
/// Caller-owned funded recovery state retains this inline value through the actual job terminal.
/// A later contiguous step needs a fresh genuine operation, preserving each original archive.
#[must_use = "Retain actual migration work through checked repository retirement and terminal delivery"]
pub struct SummaryMigrationWork {
    operation: LogicalStoreOperationKey,
    original_read: Option<usize>,
    recovery_journal: Option<DiagnosticReadLease>,
    recovery_summary: Option<DiagnosticReadLease>,
    preparation: Option<DiagnosticSummaryMigrationPreparation>,
    journal: Option<SummaryMigrationJournal>,
    journal_stage: Option<DiagnosticSummaryMigrationJournalStage>,
    permit: Option<DiagnosticSummaryMigrationPermit>,
    activation: Option<DiagnosticActivation>,
    archive: Option<DiagnosticSummaryMigrationArchiveCommit>,
    completed: bool,
}
impl SummaryMigrationWork {
    /// Actual inline retained metadata required by the composing recovery source, in bytes.
    pub const fn storage_bytes() -> usize {
        size_of::<Self>()
    }
    /// Return the actual operation whose native inputs were admitted before effects.
    pub const fn operation(&self) -> LogicalStoreOperationKey {
        self.operation
    }
    /// Observe complete archive activation and checked closure of the old active read.
    /// The whole repository still needs its ordinary checked retirement before another job.
    pub fn completed(&self) -> bool {
        self.completed
            && self.archive.as_ref().is_some_and(|archive| archive.operation() == self.operation)
    }
}
impl IncidentRepository {
    /// Migrate at most one legacy Summary by one supported contiguous format step.
    /// Original bytes are copied, flushed and journaled before exact candidate activation;
    /// archive settlement preserves that untouched original across later steps/restarts.
    /// Runs only inside the original finite execution job and actual protected writer operation.
    /// # Errors
    /// Rejects repeat use, malformed/unknown schemas or changed source facts. Actual partial
    /// reads/stages/intent remain in this repository and supplied work for checked cleanup.
    pub fn migrate_one_pending_summary(
        &mut self,
        work: &mut Option<SummaryMigrationWork>,
        context: &JobContext,
    ) -> Result<bool, IncidentStoreError> {
        if work.is_some()
            || self.pending.is_some()
            || self.preflight.is_some()
            || self.initialization.is_some()
            || !self.reads.is_empty()
            || !self.stages.is_empty()
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let inventory = self.inventory(context)?;
        if self.reconcile_summary_migrations(&inventory, work, context)? {
            return Ok(true);
        }
        self.storage.validate_preserved_migration_inventory(&self.operation, context)?;
        for candidate in
            inventory.entries().filter(|entry| entry.class() == DiagnosticEntryClass::Summary)
        {
            let (bytes, _, index) = self.read_candidate(candidate, context)?;
            let mut output = self.storage.record_buffer(&self.operation, context)?;
            let selected = {
                let mut decode = self.storage.recovery_decode(&self.operation, context)?;
                super::allocation::decode(&mut decode, || {
                    let Some((step, stored, converted)) =
                        super::codec_summary::migration_candidate(bytes.as_bytes())?
                    else {
                        return Ok(None);
                    };
                    output
                        .write_all(&converted)
                        .map_err(|cause| IncidentStoreError::Codec(serde_json::Error::io(cause)))?;
                    Ok(Some((
                        step,
                        stored.summary().seed().key(),
                        stored.summary().revision(),
                        stored.payload_revision(),
                    )))
                })?
            };
            let Some((step, key, revision, payload)) = selected else {
                drop(output);
                drop(bytes);
                self.close_read(index, context)?;
                continue;
            };
            self.namespace(key)?;
            self.reads.get_mut(index)?.bind_object(DiagnosticObject::Summary(key), context)?;
            drop(bytes);
            *work = Some(SummaryMigrationWork {
                operation: self.operation(),
                original_read: Some(index),
                recovery_journal: None,
                recovery_summary: None,
                preparation: None,
                journal: None,
                journal_stage: None,
                permit: None,
                activation: None,
                archive: None,
                completed: false,
            });
            let work = work.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
            work.preparation = Some(self.storage.prepare_summary_migration(
                self.reads.get_mut(index)?,
                &self.operation,
                context,
            )?);
            let prepared = work.preparation.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
            prepared.write_candidate(output.as_bytes(), context)?;
            let actual = prepared.flush_candidate(context)?.read_facts();
            drop(output);
            work.journal = Some(SummaryMigrationJournal::for_step(
                step,
                self.operation(),
                key,
                revision,
                payload,
                facts(prepared.original_file())?,
                facts(prepared.original_stage().read_facts())?,
                facts(actual)?,
            )?);
            let journal = work.journal.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?;
            let mut journal_bytes = self.storage.record_buffer(&self.operation, context)?;
            {
                let mut decode = self.storage.recovery_decode(&self.operation, context)?;
                super::allocation::decode(&mut decode, || {
                    super::codec_ledger::write_store_state(
                        &IncidentStoreState::SummaryMigration(Box::new(journal.clone())),
                        &mut journal_bytes,
                    )
                })?;
            }
            let plan = DiagnosticSummaryMigrationPlan::for_step(
                step,
                self.operation(),
                key,
                revision,
                payload,
                recorded(journal.original_file())?,
                recorded(journal.original_stage())?,
                recorded(journal.candidate_stage())?,
                digest(journal_bytes.as_bytes()),
            )?;
            work.journal_stage = Some(self.storage.seal_summary_migration(
                prepared,
                plan,
                &IncidentSummaryMigrationValidator,
                &self.operation,
                context,
            )?);
            let stage = work.journal_stage.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
            stage.write_chunk(journal_bytes.as_bytes(), context)?;
            stage.flush(context)?;
            drop(journal_bytes);
            work.permit =
                Some(self.storage.activate_summary_migration(stage, &self.operation, context)?);
            self.finish_summary_migration_work(work, context)?;
            self.close_read(
                work.original_read.ok_or(IncidentStoreError::RecoveryConflict)?,
                context,
            )?;
            work.original_read = None;
            work.completed = true;
            return Ok(true);
        }
        Ok(false)
    }
    /// Activate/readback the exact candidate and archive the original through the sole native owner.
    fn finish_summary_migration_work(
        &mut self,
        work: &mut SummaryMigrationWork,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        let journal = work.journal.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?;
        let permit = work.permit.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        work.activation = Some(self.storage.complete_summary_migration(
            permit,
            &IncidentSummaryMigrationValidator,
            &self.operation,
            context,
        )?);
        let mut archive_bytes = self.storage.record_buffer(&self.operation, context)?;
        {
            let mut decode = self.storage.recovery_decode(&self.operation, context)?;
            super::allocation::decode(&mut decode, || {
                super::codec_ledger::write_store_state(
                    &IncidentStoreState::SummaryMigrationArchive(Box::new(
                        SummaryMigrationArchive::new(journal.clone())?,
                    )),
                    &mut archive_bytes,
                )
            })?;
        }
        work.archive = Some(self.storage.finish_summary_migration(
            permit,
            work.activation.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?,
            archive_bytes.as_bytes(),
            &IncidentSummaryMigrationValidator,
            &self.operation,
            context,
        )?);
        drop(archive_bytes);
        Ok(())
    }
}
/// Retain only complete actual source identity/digest/length as immutable expectations.
fn facts(actual: DiagnosticReadFacts) -> Result<IncidentFileFacts, IncidentStoreError> {
    IncidentFileFacts::new(actual.file_identity(), actual.sha256(), actual.bytes())
}
/// Convert matching expectations without producing any storage/native authority.
fn recorded(actual: IncidentFileFacts) -> Result<RecordedDiagnosticFileFacts, IncidentStoreError> {
    RecordedDiagnosticFileFacts::new(actual.identity(), actual.sha256(), actual.bytes())
        .map_err(Into::into)
}
