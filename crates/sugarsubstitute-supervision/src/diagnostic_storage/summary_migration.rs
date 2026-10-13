// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve legacy Summary bytes before sealed canonical schema2 activation.
mod archive;
pub(super) mod intent;
mod recovery;
mod types;
use super::stage::slots::*;
use super::*;
use intent::*;
use source::{canonical, read_by_id, retained_source, stage_by_id, whole};
pub use types::*;
pub(super) use types::{MigrationArchive, MigrationIntent, MigrationRole};
impl DiagnosticStorage {
    /// Preserve an untouched legacy Summary in two privately owned bounded stages.
    /// Runs on an admitted worker under original execution context and writer exclusion.
    /// # Errors
    /// Foreign sources, incomplete classification, saturated archives/stages, expiry
    /// and partial native effects preserve actual outputs and their byte claims.
    pub fn prepare_summary_migration(
        &mut self,
        original: &mut DiagnosticReadLease,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticSummaryMigrationPreparation, DiagnosticStorageError> {
        let source = whole(&self.shared, original, operation, context)?;
        let mut state = self.check(operation, context)?;
        stage::mutation(&mut state, operation.key)?;
        classified(&mut state, operation.key)?;
        let Some(object @ DiagnosticObject::Summary(_)) = source.object else {
            return Err(error(operation.key, Code::InvalidIdentity));
        };
        let namespace =
            state.data_mut()?.namespace.ok_or_else(|| error(operation.key, Code::Closed))?;
        if !canonical(&source, object, namespace)
            || state.data_mut()?.deletion.is_some()
            || state.data_mut()?.archives.len() >= DIAGNOSTIC_MIGRATION_ARCHIVE_BOUND
            || state.data_mut()?.stages.len() + 2 > DIAGNOSTIC_STAGE_BOUND
        {
            return Err(error(operation.key, Code::Capacity));
        }
        retained_source(&mut state, &source, operation.key, context)?;
        let id = next_lease(&mut state, operation.key)?;
        let reservation =
            allocation::reservation(&state.data_mut()?.allocation, Some(operation.key))?;
        state
            .data_mut()?
            .archives
            .try_reserve_exact(1)
            .map_err(|cause| reservation.failed(cause))?;
        let allocation =
            allocation::migration_intent(&state.data_mut()?.allocation, operation.key)?;
        state.data_mut()?.migration =
            Some(allocation.migration_intent(MigrationIntent::new(id, object)));
        migration(&mut state, id, operation.key)?.original_read = Some(original.id);
        let copy = reserve_stage(
            &mut state,
            object,
            stage::StageRole::Migration(MigrationRole::Original),
            DIAGNOSTIC_IO_BYTES as u64,
            operation.key,
        )?;
        migration(&mut state, id, operation.key)?.original_stage = Some(copy);
        let candidate = reserve_stage(
            &mut state,
            object,
            stage::StageRole::Migration(MigrationRole::Candidate),
            DIAGNOSTIC_IO_BYTES as u64,
            operation.key,
        )?;
        migration(&mut state, id, operation.key)?.candidate_stage = Some(candidate);
        create_stage(&mut state, copy, operation.key, context)?;
        create_stage(&mut state, candidate, operation.key, context)?;
        append_stage(&mut state, copy, &source.bytes, operation.key, context)?;
        let facts = flush_stage(&mut state, copy, operation.key, context)?;
        if facts.sha256 != source.facts.sha256
            || facts.bytes != source.facts.bytes
            || facts.identity == source.facts.identity
        {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        Ok(DiagnosticSummaryMigrationPreparation {
            shared: self.shared.clone(),
            operation: operation.key,
            id,
            original: source.facts,
            copy: DiagnosticStageFacts(facts),
        })
    }
    /// Seal exact original/candidate bytes before creating the durable intent Journal.
    /// The codec callback has immutable actual sources and runs outside the owner mutex.
    /// # Errors
    /// Callback/context/source discrepancies leave sealed custody intact for cleanup.
    pub fn seal_summary_migration(
        &mut self,
        preparation: &mut DiagnosticSummaryMigrationPreparation,
        plan: DiagnosticSummaryMigrationPlan,
        validator: &dyn DiagnosticSummaryMigrationValidator,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticSummaryMigrationJournalStage, DiagnosticStorageError> {
        pin(&self.shared, &preparation.shared, preparation.operation, operation)?;
        let (original, copy, candidate) = {
            let mut state = self.check(operation, context)?;
            let intent = migration(&mut state, preparation.id, operation.key)?;
            if intent.validating
                || intent.plan.is_some()
                || plan.historical != operation.key
                || intent.object != DiagnosticObject::Summary(plan.incident)
            {
                return Err(error(operation.key, Code::InvalidIdentity));
            }
            intent.validating = true;
            (
                required(intent.original_read, operation.key)?,
                required(intent.original_stage, operation.key)?,
                required(intent.candidate_stage, operation.key)?,
            )
        };
        let original = read_by_id(self, operation, original, context)?;
        let copy = stage_by_id(self, operation, copy, context)?;
        let candidate = stage_by_id(self, operation, candidate, context)?;
        if !plan.original_file.matches(original.facts)
            || !plan.original_stage.matches(copy.facts)
            || !plan.candidate_stage.matches(candidate.facts)
        {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        allocation::decode_callback(self, operation, context, |decode| {
            validator.validate_preparation(
                decode,
                operation.key,
                &plan,
                &original,
                &copy,
                &candidate,
            )
        })?;
        let mut state = self.check(operation, context)?;
        revalidate(&mut state, &[&original, &copy, &candidate], operation.key, context)?;
        let intent = migration(&mut state, preparation.id, operation.key)?;
        if !intent.validating {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        intent.plan = Some(plan);
        intent.validating = false;
        intent.semantic_verified = true;
        let journal = reserve_stage(
            &mut state,
            DiagnosticObject::Journal(plan.historical),
            stage::StageRole::Migration(MigrationRole::Journal),
            DIAGNOSTIC_IO_BYTES as u64,
            operation.key,
        )?;
        migration(&mut state, preparation.id, operation.key)?.journal_stage = Some(journal);
        create_stage(&mut state, journal, operation.key, context)?;
        Ok(DiagnosticSummaryMigrationJournalStage {
            shared: self.shared.clone(),
            operation: operation.key,
            id: preparation.id,
        })
    }
    /// Activate the sealed canonical Journal and retain its actual read custody.
    /// # Errors
    /// Uncertain activation/readback/close preserves every actual output and intent.
    pub fn activate_summary_migration(
        &mut self,
        journal: &mut DiagnosticSummaryMigrationJournalStage,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticSummaryMigrationPermit, DiagnosticStorageError> {
        pin(&self.shared, &journal.shared, journal.operation, operation)?;
        let plan = {
            let mut state = self.check(operation, context)?;
            let intent = migration(&mut state, journal.id, operation.key)?;
            let plan = intent.plan.ok_or_else(|| error(operation.key, Code::Closed))?;
            if intent.validating || intent.journal_read.is_some() {
                return Err(error(operation.key, Code::Busy));
            }
            intent.journal_attempted = true;
            if intent.journal_facts.is_none() {
                let id = required(intent.journal_stage, operation.key)?;
                let expected = stage_facts(&mut state, id, operation.key)?;
                if expected.sha256 != plan.journal_sha256 {
                    return Err(error(operation.key, Code::ChangedIdentity));
                }
                let facts = settle_stage(
                    &mut state,
                    id,
                    DiagnosticPredecessor::Absent,
                    operation.key,
                    context,
                )?;
                let intent = migration(&mut state, journal.id, operation.key)?;
                intent.journal_stage = None;
                intent.journal_facts = Some(facts);
            }
            plan
        };
        let mut lease = self.read(
            DiagnosticObject::Journal(plan.historical),
            DIAGNOSTIC_IO_BYTES as u64,
            operation,
            context,
        )?;
        let source = whole(&self.shared, &mut lease, operation, context)?;
        let mut state = self.check(operation, context)?;
        let intent = migration(&mut state, journal.id, operation.key)?;
        if intent.journal_read.is_some()
            || intent.journal_facts != Some(source.facts)
            || source.facts.sha256 != plan.journal_sha256
        {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        intent.journal_read = Some(lease.id);
        Ok(DiagnosticSummaryMigrationPermit {
            shared: self.shared.clone(),
            current: operation.key,
            id: journal.id,
            plan,
        })
    }
}
impl DiagnosticSummaryMigrationPreparation {
    /// Append bounded candidate bytes while the preparation remains unsealed.
    /// # Errors
    /// Sealed/foreign/expired preparation or native failure preserves candidate custody.
    pub fn write_candidate(
        &mut self,
        bytes: &[u8],
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        let mut state = StateAccess::acquire(&self.shared, self.operation);
        check(&mut state, self.operation, context, false)?;
        let intent = migration(&mut state, self.id, self.operation)?;
        if intent.validating || intent.plan.is_some() {
            return Err(error(self.operation, Code::Busy));
        }
        let id = required(intent.candidate_stage, self.operation)?;
        append_stage(&mut state, id, bytes, self.operation, context)
    }
    /// Flush and verify the actual complete candidate stream before sealing.
    /// # Errors
    /// Rejects sealed/expired preparation and preserves failed native flush/readback.
    pub fn flush_candidate(
        &mut self,
        context: &JobContext,
    ) -> Result<DiagnosticStageFacts, DiagnosticStorageError> {
        let mut state = StateAccess::acquire(&self.shared, self.operation);
        check(&mut state, self.operation, context, false)?;
        let intent = migration(&mut state, self.id, self.operation)?;
        if intent.validating || intent.plan.is_some() {
            return Err(error(self.operation, Code::Busy));
        }
        let id = required(intent.candidate_stage, self.operation)?;
        Ok(DiagnosticStageFacts(flush_stage(&mut state, id, self.operation, context)?))
    }
}
impl DiagnosticSummaryMigrationJournalStage {
    /// Append only the sealed bounded intent bytes; ordinary stage authority is absent.
    /// # Errors
    /// Rejects retired/activated/foreign intent and retains partial writes.
    pub fn write_chunk(
        &mut self,
        bytes: &[u8],
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        let mut state = StateAccess::acquire(&self.shared, self.operation);
        check(&mut state, self.operation, context, false)?;
        let intent = migration(&mut state, self.id, self.operation)?;
        if intent.validating || intent.journal_attempted {
            return Err(error(self.operation, Code::Busy));
        }
        let id = required(intent.journal_stage, self.operation)?;
        append_stage(&mut state, id, bytes, self.operation, context)
    }
    /// Verify the actual complete Journal digest matches the sealed canonical intent.
    /// # Errors
    /// Mismatched bytes or failed flush retain the native stage and reservation.
    pub fn flush(
        &mut self,
        context: &JobContext,
    ) -> Result<DiagnosticStageFacts, DiagnosticStorageError> {
        let mut state = StateAccess::acquire(&self.shared, self.operation);
        check(&mut state, self.operation, context, false)?;
        let intent = migration(&mut state, self.id, self.operation)?;
        let id = required(intent.journal_stage, self.operation)?;
        let expected =
            intent.plan.ok_or_else(|| error(self.operation, Code::Closed))?.journal_sha256;
        let facts = flush_stage(&mut state, id, self.operation, context)?;
        if facts.sha256 != expected {
            return Err(error(self.operation, Code::ChangedIdentity));
        }
        Ok(DiagnosticStageFacts(facts))
    }
}
