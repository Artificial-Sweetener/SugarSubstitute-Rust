// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Settle one immutable Archive stage and recognize its preserved original-copy.
use super::*;
use sha2::{Digest, Sha256};
impl DiagnosticStorage {
    /// Replace the exact historical intent Journal with its inert checked Archive.
    /// Original-copy bytes remain on disk and become recognized only after checked closure.
    /// # Errors
    /// Changed bytes/generations, callback failure and uncertain effects retain the same latched stage.
    pub fn finish_summary_migration(
        &mut self,
        permit: &mut DiagnosticSummaryMigrationPermit,
        candidate: &DiagnosticActivation,
        archive_bytes: &[u8],
        validator: &dyn DiagnosticSummaryMigrationValidator,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticSummaryMigrationArchiveCommit, DiagnosticStorageError> {
        pin(&self.shared, &permit.shared, permit.current, operation)?;
        if !Shared::ptr_eq(&self.shared, &candidate.shared)
            || candidate.operation != operation.key
            || candidate.object != DiagnosticObject::Summary(permit.plan.incident)
            || !permit.plan.candidate_stage.matches(candidate.facts)
            || archive_bytes.is_empty()
            || archive_bytes.len() > DIAGNOSTIC_IO_BYTES
        {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let digest: [u8; 32] = Sha256::digest(archive_bytes).into();
        {
            let mut state = self.check(operation, context)?;
            let intent = migration(&mut state, permit.id, operation.key)?;
            if intent.archive_bytes.as_ref().is_some_and(|bytes| {
                bytes.as_slice() != archive_bytes || intent.archive_sha256 != Some(digest)
            }) {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
        }
        let actual = self.complete_summary_migration(permit, validator, operation, context)?;
        if actual.facts != candidate.facts {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        let stage = {
            let mut state = self.check(operation, context)?;
            let owner = state.data_mut()?.allocation.clone();
            let intent = migration(&mut state, permit.id, operation.key)?;
            if intent.validating
                || intent.plan != Some(permit.plan)
                || intent.candidate_active != Some(candidate.facts)
            {
                return Err(error(operation.key, Code::Busy));
            }
            if let Some(bytes) = &intent.archive_bytes {
                if bytes.as_slice() != archive_bytes || intent.archive_sha256 != Some(digest) {
                    return Err(error(operation.key, Code::ChangedIdentity));
                }
            } else {
                let reservation = allocation::reservation(&owner, Some(operation.key))?;
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(archive_bytes.len())
                    .map_err(|cause| reservation.failed(cause))?;
                bytes.extend_from_slice(archive_bytes);
                intent.archive_bytes = Some(bytes);
                intent.archive_sha256 = Some(digest);
            }
            if intent.archive_active.is_some() {
                None
            } else if let Some(id) = intent.archive_stage {
                Some(id)
            } else {
                let id = reserve_stage(
                    &mut state,
                    DiagnosticObject::Journal(permit.plan.historical),
                    stage::StageRole::Migration(MigrationRole::Archive),
                    DIAGNOSTIC_IO_BYTES as u64,
                    operation.key,
                )?;
                migration(&mut state, permit.id, operation.key)?.archive_stage = Some(id);
                create_stage(&mut state, id, operation.key, context)?;
                append_stage(&mut state, id, archive_bytes, operation.key, context)?;
                flush_stage(&mut state, id, operation.key, context)?;
                Some(id)
            }
        };
        if let Some(stage) = stage {
            validate_archive_stage(self, permit, stage, validator, operation, context)?;
        }
        let mut state = self.check(operation, context)?;
        let intent = migration(&mut state, permit.id, operation.key)?;
        let journal = intent.journal_read;
        let original = required(intent.original_stage, operation.key)?;
        let old = intent.journal_facts.ok_or_else(|| error(operation.key, Code::Closed))?;
        if let Some(journal) = journal {
            close_read(&mut state, journal, operation.key, context)?;
            migration(&mut state, permit.id, operation.key)?.journal_read = None;
        }
        let archive = if let Some(stage) = stage {
            let facts = settle_stage(&mut state, stage, predecessor(old), operation.key, context)?;
            let intent = migration(&mut state, permit.id, operation.key)?;
            intent.archive_stage = None;
            intent.archive_active = Some(facts);
            facts
        } else {
            let facts = migration(&mut state, permit.id, operation.key)?
                .archive_active
                .ok_or_else(|| error(operation.key, Code::Closed))?;
            let actual = state
                .call_native(|port| {
                    port.reconcile_activation(
                        DiagnosticObject::Journal(permit.plan.historical),
                        predecessor(facts),
                        context,
                    )
                })?
                .map_err(|cause| native(operation.key, Code::ActivationUncertain, cause))?;
            if actual != facts {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
            check(&mut state, operation.key, context, false)?;
            actual
        };
        if archive.sha256 != digest {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        let copy = stage_facts(&mut state, original, operation.key)?;
        if !permit.plan.original_stage.matches(copy) {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        close_stage(&mut state, original, operation.key, context)?;
        recognize(&mut state, permit.plan, archive, copy, operation.key)?;
        state.data_mut()?.migration = None;
        state.data_mut()?.inventory_complete = false;
        Ok(DiagnosticSummaryMigrationArchiveCommit {
            current: operation.key,
            historical: permit.plan.historical,
            facts: archive,
        })
    }
    /// Recognize a strict historical Archive from actual bytes and the retained original-copy.
    /// This operation grants no Summary mutation, deletion or historical replay authority.
    /// # Errors
    /// Unknown/missing/changed original copies, saturated archives and failed outputs remain preserved.
    pub fn recognize_summary_migration_archive(
        &mut self,
        archive: &mut DiagnosticReadLease,
        validator: &dyn DiagnosticSummaryMigrationValidator,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticSummaryMigrationArchiveObservation, DiagnosticStorageError> {
        let source = whole(&self.shared, archive, operation, context)?;
        let id = {
            let mut state = self.check(operation, context)?;
            stage::mutation(&mut state, operation.key)?;
            if state.data_mut()?.deletion.is_some() {
                return Err(error(operation.key, Code::Busy));
            }
            let id = next_lease(&mut state, operation.key)?;
            let allocation =
                allocation::migration_intent(&state.data_mut()?.allocation, operation.key)?;
            let mut intent = MigrationIntent::new(id, DiagnosticObject::StoreState);
            intent.validating = true;
            intent.journal_read = Some(archive.id);
            state.data_mut()?.migration = Some(allocation.migration_intent(intent));
            id
        };
        let decision = allocation::decode_callback(self, operation, context, |decode| {
            validator.validate_archive(decode, operation.key, &source)
        })?;
        let plan = decision.plan;
        let original = {
            let mut state = self.check(operation, context)?;
            revalidate(&mut state, &[&source], operation.key, context)?;
            if plan.historical.owner().namespace != operation.key.owner().namespace
                || decision.archive_sha256 != source.facts.sha256
                || !canonical(
                    &source,
                    DiagnosticObject::Journal(plan.historical),
                    operation.key.owner().namespace,
                )
            {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
            reserve_archive(&mut state, plan.historical, operation.key)?;
            if let Some(known) =
                state.data_mut()?.archives.iter().find(|known| known.historical == plan.historical)
                && (known.archive != source.facts || !plan.original_stage.matches(known.original))
            {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
            let original = adopt_stage(
                &mut state,
                DiagnosticObject::Summary(plan.incident),
                &plan,
                MigrationRole::Original,
                operation.key,
                context,
            )?;
            let intent = migration(&mut state, id, operation.key)?;
            intent.object = DiagnosticObject::Summary(plan.incident);
            intent.plan = Some(plan);
            intent.original_stage = Some(original);
            original
        };
        let copy = stage_by_id(self, operation, original, context)?;
        allocation::decode_callback(self, operation, context, |decode| {
            validator.validate_archive_original(decode, operation.key, &plan, &copy)
        })?;
        let mut state = self.check(operation, context)?;
        revalidate(&mut state, &[&source, &copy], operation.key, context)?;
        close_stage(&mut state, original, operation.key, context)?;
        close_read(&mut state, archive.id, operation.key, context)?;
        recognize(&mut state, plan, source.facts, copy.facts, operation.key)?;
        state.data_mut()?.migration = None;
        Ok(DiagnosticSummaryMigrationArchiveObservation {
            current: operation.key,
            historical: plan.historical,
            original: copy.facts,
        })
    }
}
fn validate_archive_stage(
    storage: &mut DiagnosticStorage,
    permit: &DiagnosticSummaryMigrationPermit,
    id: u64,
    validator: &dyn DiagnosticSummaryMigrationValidator,
    operation: &DiagnosticStorageOperation,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    {
        let mut state = storage.check(operation, context)?;
        migration(&mut state, permit.id, operation.key)?.validating = true;
    }
    let live = {
        let mut state = storage.check(operation, context)?;
        let index = stage_index(&mut state, id, operation.key)?;
        if state.data_mut()?.stages[index].native.is_some()
            && state.data_mut()?.stages[index].flushed.is_none()
        {
            let written = state.data_mut()?.stages[index].written;
            let expected = migration(&mut state, permit.id, operation.key)?
                .archive_bytes
                .as_ref()
                .map(Vec::len)
                .ok_or_else(|| error(operation.key, Code::Closed))?;
            if written != expected as u64 {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
            flush_stage(&mut state, id, operation.key, context)?;
        }
        state.data_mut()?.stages[index].native.is_some()
    };
    let source = if live {
        stage_by_id(storage, operation, id, context)?
    } else {
        let mut lease = storage.read(
            DiagnosticObject::Journal(permit.plan.historical),
            DIAGNOSTIC_IO_BYTES as u64,
            operation,
            context,
        )?;
        {
            let mut state = storage.check(operation, context)?;
            migration(&mut state, permit.id, operation.key)?.journal_read = Some(lease.id);
        }
        whole(&storage.shared, &mut lease, operation, context)?
    };
    let decision = allocation::decode_callback(storage, operation, context, |decode| {
        validator.validate_archive(decode, operation.key, &source)
    })?;
    let mut state = storage.check(operation, context)?;
    revalidate(&mut state, &[&source], operation.key, context)?;
    let intent = migration(&mut state, permit.id, operation.key)?;
    if decision.plan != permit.plan
        || decision.archive_sha256 != source.facts.sha256
        || intent.archive_sha256 != Some(source.facts.sha256)
    {
        return Err(error(operation.key, Code::ChangedIdentity));
    }
    intent.validating = false;
    Ok(())
}
