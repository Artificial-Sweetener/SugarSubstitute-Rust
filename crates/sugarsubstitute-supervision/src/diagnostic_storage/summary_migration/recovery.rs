// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover exact historical stages under newly admitted authority and preserve their bytes.
use super::*;
impl DiagnosticStorage {
    /// Recover a strict intent from retained protected Journal and Summary sources.
    /// The original-copy is opened readonly before returning any current authority.
    /// # Errors
    /// Unknown generations, names, stages, callbacks and failed native outputs preserve custody.
    pub fn recover_summary_migration(
        &mut self,
        journal: &mut DiagnosticReadLease,
        summary: &mut DiagnosticReadLease,
        validator: &dyn DiagnosticSummaryMigrationValidator,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticSummaryMigrationPermit, DiagnosticStorageError> {
        let journal_source = whole(&self.shared, journal, operation, context)?;
        let summary_source = whole(&self.shared, summary, operation, context)?;
        let id = {
            let mut state = self.check(operation, context)?;
            stage::mutation(&mut state, operation.key)?;
            if state.data_mut()?.deletion.is_some() {
                return Err(error(operation.key, Code::Busy));
            }
            let id = next_lease(&mut state, operation.key)?;
            let allocation =
                allocation::migration_intent(&state.data_mut()?.allocation, operation.key)?;
            let mut intent = MigrationIntent::new(
                id,
                summary_source.object.unwrap_or(DiagnosticObject::StoreState),
            );
            intent.journal_read = Some(journal.id);
            intent.original_read = Some(summary.id);
            intent.validating = true;
            state.data_mut()?.migration = Some(allocation.migration_intent(intent));
            id
        };
        let decision = allocation::decode_callback(self, operation, context, |decode| {
            validator.validate_recovery(decode, operation.key, &journal_source, &summary_source)
        })?;
        let plan = decision.plan;
        {
            let mut state = self.check(operation, context)?;
            revalidate(&mut state, &[&journal_source, &summary_source], operation.key, context)?;
            let namespace = operation.key.owner().namespace;
            let expected = match decision.case {
                DiagnosticSummaryMigrationCase::Original => plan.original_file,
                DiagnosticSummaryMigrationCase::Candidate => plan.candidate_stage,
            };
            if plan.historical.owner().namespace != namespace
                || journal_source.facts.sha256 != plan.journal_sha256
                || !canonical(
                    &journal_source,
                    DiagnosticObject::Journal(plan.historical),
                    namespace,
                )
                || !canonical(&summary_source, DiagnosticObject::Summary(plan.incident), namespace)
                || !expected.matches(summary_source.facts)
            {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
            reserve_archive(&mut state, plan.historical, operation.key)?;
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
            intent.original_stage = Some(original);
            intent.journal_facts = Some(journal_source.facts);
            intent.journal_attempted = true;
            intent.plan = Some(plan);
            intent.case = decision.case;
            intent.validating = false;
        }
        Ok(DiagnosticSummaryMigrationPermit {
            shared: self.shared.clone(),
            current: operation.key,
            id,
            plan,
        })
    }
    /// Adopt the exact existing candidate only for the untouched Original case.
    /// No candidate bytes are created, rewritten or charged twice.
    /// # Errors
    /// Candidate/foreign/changed state or failed semantic/native checks retains actual outputs.
    pub fn recover_summary_migration_candidate(
        &mut self,
        permit: &mut DiagnosticSummaryMigrationPermit,
        validator: &dyn DiagnosticSummaryMigrationValidator,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        pin(&self.shared, &permit.shared, permit.current, operation)?;
        {
            let mut state = self.check(operation, context)?;
            let intent = migration(&mut state, permit.id, operation.key)?;
            if intent.validating
                || intent.plan != Some(permit.plan)
                || intent.case != DiagnosticSummaryMigrationCase::Original
            {
                return Err(error(operation.key, Code::Busy));
            }
            if intent.candidate_stage.is_none() {
                let id = adopt_stage(
                    &mut state,
                    DiagnosticObject::Summary(permit.plan.incident),
                    &permit.plan,
                    MigrationRole::Candidate,
                    operation.key,
                    context,
                )?;
                migration(&mut state, permit.id, operation.key)?.candidate_stage = Some(id);
            }
        }
        verify_conversion(self, permit, validator, operation, context)
    }
    /// Settle the exact candidate under the original context and retained intent.
    /// Candidate retries reconcile the actual inode; Original alone may perform CAS.
    /// # Errors
    /// Third states, cancellation and uncertain activation/closure preserve every native output.
    pub fn complete_summary_migration(
        &mut self,
        permit: &mut DiagnosticSummaryMigrationPermit,
        validator: &dyn DiagnosticSummaryMigrationValidator,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticActivation, DiagnosticStorageError> {
        pin(&self.shared, &permit.shared, permit.current, operation)?;
        verify_conversion(self, permit, validator, operation, context)?;
        let mut state = self.check(operation, context)?;
        let intent = migration(&mut state, permit.id, operation.key)?;
        if intent.plan != Some(permit.plan) || !intent.semantic_verified {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let read = intent.original_read;
        let stage = intent.candidate_stage;
        if let Some(read) = read {
            close_read(&mut state, read, operation.key, context)?;
            migration(&mut state, permit.id, operation.key)?.original_read = None;
        }
        let actual = if let Some(stage) = stage {
            settle_stage(
                &mut state,
                stage,
                DiagnosticPredecessor::Exact {
                    file_identity: permit.plan.original_file.identity,
                    sha256: permit.plan.original_file.sha256,
                },
                operation.key,
                context,
            )?
        } else {
            let expected = permit.plan.candidate_stage;
            let actual = state
                .call_native(|port| {
                    port.reconcile_activation(
                        DiagnosticObject::Summary(permit.plan.incident),
                        DiagnosticPredecessor::Exact {
                            file_identity: expected.identity,
                            sha256: expected.sha256,
                        },
                        context,
                    )
                })?
                .map_err(|cause| native(operation.key, Code::ActivationUncertain, cause))?;
            if !expected.matches(actual) {
                return Err(error(operation.key, Code::ChangedIdentity));
            }
            check(&mut state, operation.key, context, false)?;
            actual
        };
        if !permit.plan.candidate_stage.matches(actual) {
            return Err(error(operation.key, Code::ChangedIdentity));
        }
        let intent = migration(&mut state, permit.id, operation.key)?;
        intent.candidate_stage = None;
        intent.candidate_active = Some(actual);
        intent.case = DiagnosticSummaryMigrationCase::Candidate;
        Ok(DiagnosticActivation {
            shared: self.shared.clone(),
            operation: operation.key,
            object: DiagnosticObject::Summary(permit.plan.incident),
            facts: actual,
        })
    }
}
fn verify_conversion(
    storage: &mut DiagnosticStorage,
    permit: &DiagnosticSummaryMigrationPermit,
    validator: &dyn DiagnosticSummaryMigrationValidator,
    operation: &DiagnosticStorageOperation,
    context: &JobContext,
) -> Result<(), DiagnosticStorageError> {
    let (original, candidate, read) = {
        let mut state = storage.check(operation, context)?;
        let intent = migration(&mut state, permit.id, operation.key)?;
        if intent.validating || intent.plan != Some(permit.plan) {
            return Err(error(operation.key, Code::Busy));
        }
        intent.validating = true;
        (
            required(intent.original_stage, operation.key)?,
            intent.candidate_stage,
            intent.original_read,
        )
    };
    let original = stage_by_id(storage, operation, original, context)?;
    let candidate = if let Some(id) = candidate {
        let native_live = {
            let mut state = storage.check(operation, context)?;
            let index = stage_index(&mut state, id, operation.key)?;
            state.data_mut()?.stages[index].native.is_some()
        };
        if native_live {
            stage_by_id(storage, operation, id, context)?
        } else {
            current_summary(storage, permit, operation, context)?
        }
    } else if let Some(id) = read {
        read_by_id(storage, operation, id, context)?
    } else {
        current_summary(storage, permit, operation, context)?
    };
    if !permit.plan.original_stage.matches(original.facts)
        || !permit.plan.candidate_stage.matches(candidate.facts)
    {
        return Err(error(operation.key, Code::ChangedIdentity));
    }
    allocation::decode_callback(storage, operation, context, |decode| {
        validator.validate_recovered_stages(
            decode,
            operation.key,
            &permit.plan,
            &original,
            &candidate,
        )
    })?;
    let mut state = storage.check(operation, context)?;
    revalidate(&mut state, &[&original, &candidate], operation.key, context)?;
    let intent = migration(&mut state, permit.id, operation.key)?;
    intent.semantic_verified = true;
    intent.validating = false;
    Ok(())
}
pub(super) fn current_summary(
    storage: &mut DiagnosticStorage,
    permit: &DiagnosticSummaryMigrationPermit,
    operation: &DiagnosticStorageOperation,
    context: &JobContext,
) -> Result<DiagnosticRecoverySource, DiagnosticStorageError> {
    let mut lease = storage.read(
        DiagnosticObject::Summary(permit.plan.incident),
        DIAGNOSTIC_IO_BYTES as u64,
        operation,
        context,
    )?;
    {
        let mut state = storage.check(operation, context)?;
        migration(&mut state, permit.id, operation.key)?.original_read = Some(lease.id);
    }
    whole(&storage.shared, &mut lease, operation, context)
}
