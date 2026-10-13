// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run actual storage phases with a closed fake semantic adapter and external disk observations.
mod clocks;
mod deadline;
mod fences;
mod lifecycle;
mod preservation;
mod retry;
use super::*;
use crate::diagnostic_storage::fixture::DiagnosticFixtureFailure;
use sha2::{Digest, Sha256};
type Validation = DiagnosticRecoveryValidationError;
const LEGACY: &[u8] = b"strict legacy summary";
const CANDIDATE: &[u8] = b"canonical migrated summary";
const JOURNAL: &[u8] = b"canonical migration intent";
const ARCHIVE: &[u8] = b"canonical migration archive";
struct Validator {
    plan: DiagnosticSummaryMigrationPlan,
    hook: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}
impl Validator {
    fn new(plan: DiagnosticSummaryMigrationPlan) -> Self {
        Self { plan, hook: Mutex::new(None) }
    }
    fn exact(
        &self,
        source: &DiagnosticRecoverySource,
        expected: RecordedDiagnosticFileFacts,
        bytes: &[u8],
    ) -> Result<(), Validation> {
        if !expected.matches(source.facts()) || source.as_bytes() != bytes {
            return Err(Validation::rejected(DiagnosticRecoveryValidationCode::InvalidIdentity));
        }
        Ok(())
    }
}
impl DiagnosticSummaryMigrationValidator for Validator {
    fn validate_preparation(
        &self,
        decode: &mut DiagnosticRecoveryDecodeLoan,
        _: LogicalStoreOperationKey,
        plan: &DiagnosticSummaryMigrationPlan,
        original: &DiagnosticRecoverySource,
        copy: &DiagnosticRecoverySource,
        candidate: &DiagnosticRecoverySource,
    ) -> Result<(), Validation> {
        if plan != &self.plan
            || original.class() != DiagnosticEntryClass::Summary
            || copy.class() != DiagnosticEntryClass::Stage
            || candidate.class() != DiagnosticEntryClass::Stage
        {
            return Err(Validation::rejected(DiagnosticRecoveryValidationCode::InvalidIdentity));
        }
        self.exact(original, plan.original_file(), LEGACY)?;
        self.validate_recovered_stages(decode, plan.historical_operation(), plan, copy, candidate)
    }
    fn validate_recovery(
        &self,
        _decode: &mut DiagnosticRecoveryDecodeLoan,
        _: LogicalStoreOperationKey,
        journal: &DiagnosticRecoverySource,
        summary: &DiagnosticRecoverySource,
    ) -> Result<DiagnosticSummaryMigrationDecision, Validation> {
        if journal.as_bytes() != JOURNAL || journal.class() != DiagnosticEntryClass::Journal {
            return Err(Validation::rejected(DiagnosticRecoveryValidationCode::InvalidJournal));
        }
        let case = if self.plan.original_file().matches(summary.facts()) {
            self.exact(summary, self.plan.original_file(), LEGACY)?;
            DiagnosticSummaryMigrationCase::Original
        } else {
            self.exact(summary, self.plan.candidate_stage(), CANDIDATE)?;
            DiagnosticSummaryMigrationCase::Candidate
        };
        DiagnosticSummaryMigrationDecision::new(self.plan, case)
    }
    fn validate_recovered_stages(
        &self,
        _decode: &mut DiagnosticRecoveryDecodeLoan,
        _: LogicalStoreOperationKey,
        plan: &DiagnosticSummaryMigrationPlan,
        copy: &DiagnosticRecoverySource,
        candidate: &DiagnosticRecoverySource,
    ) -> Result<(), Validation> {
        if plan != &self.plan
            || copy.class() != DiagnosticEntryClass::Stage
            || !matches!(
                candidate.class(),
                DiagnosticEntryClass::Stage | DiagnosticEntryClass::Summary
            )
        {
            return Err(Validation::rejected(DiagnosticRecoveryValidationCode::InvalidIdentity));
        }
        self.exact(copy, plan.original_stage(), LEGACY)?;
        self.exact(candidate, plan.candidate_stage(), CANDIDATE)?;
        let hook = self.hook.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
        if let Some(hook) = hook {
            hook();
        }
        Ok(())
    }
    fn validate_archive_original(
        &self,
        _decode: &mut DiagnosticRecoveryDecodeLoan,
        _: LogicalStoreOperationKey,
        plan: &DiagnosticSummaryMigrationPlan,
        copy: &DiagnosticRecoverySource,
    ) -> Result<(), Validation> {
        if plan != &self.plan || copy.class() != DiagnosticEntryClass::Stage {
            return Err(Validation::rejected(DiagnosticRecoveryValidationCode::InvalidIdentity));
        }
        self.exact(copy, plan.original_stage(), LEGACY)
    }
    fn validate_archive(
        &self,
        _decode: &mut DiagnosticRecoveryDecodeLoan,
        _: LogicalStoreOperationKey,
        source: &DiagnosticRecoverySource,
    ) -> Result<DiagnosticSummaryMigrationArchiveDecision, Validation> {
        if source.as_bytes() != ARCHIVE
            || !matches!(
                source.class(),
                DiagnosticEntryClass::Stage | DiagnosticEntryClass::Journal
            )
        {
            return Err(Validation::rejected(DiagnosticRecoveryValidationCode::InvalidJournal));
        }
        DiagnosticSummaryMigrationArchiveDecision::new(self.plan, Sha256::digest(ARCHIVE).into())
    }
}
fn recorded(
    facts: DiagnosticReadFacts,
) -> Result<RecordedDiagnosticFileFacts, DiagnosticStorageError> {
    RecordedDiagnosticFileFacts::new(facts.file_identity(), facts.sha256(), facts.bytes())
}
fn plan(
    historical: LogicalStoreOperationKey,
    original: DiagnosticReadFacts,
    copy: DiagnosticReadFacts,
    candidate: DiagnosticReadFacts,
) -> Result<DiagnosticSummaryMigrationPlan, TestError> {
    Ok(DiagnosticSummaryMigrationPlan::new(
        historical,
        incident()?,
        IncidentRevision::new(1)?,
        None,
        recorded(original)?,
        recorded(copy)?,
        recorded(candidate)?,
        Sha256::digest(JOURNAL).into(),
    )?)
}
fn normal(
    storage: &mut DiagnosticStorage,
    operation: &DiagnosticStorageOperation,
    context: &JobContext,
) -> Result<(DiagnosticSummaryMigrationPermit, Validator), TestError> {
    let (permit, validator, _) = normal_with_read(storage, operation, context)?;
    Ok((permit, validator))
}
fn normal_with_read(
    storage: &mut DiagnosticStorage,
    operation: &DiagnosticStorageOperation,
    context: &JobContext,
) -> Result<(DiagnosticSummaryMigrationPermit, Validator, DiagnosticReadLease), TestError> {
    let mut original =
        storage.read(DiagnosticObject::Summary(incident()?), 64, operation, context)?;
    let mut preparation = storage.prepare_summary_migration(&mut original, operation, context)?;
    preparation.write_candidate(CANDIDATE, context)?;
    let candidate = preparation.flush_candidate(context)?;
    let plan = plan(
        operation.key,
        preparation.original_file(),
        preparation.original_stage().read_facts(),
        candidate.read_facts(),
    )?;
    let validator = Validator::new(plan);
    let mut journal =
        storage.seal_summary_migration(&mut preparation, plan, &validator, operation, context)?;
    journal.write_chunk(JOURNAL, context)?;
    journal.flush(context)?;
    Ok((storage.activate_summary_migration(&mut journal, operation, context)?, validator, original))
}
fn recovery(filesystem: &Filesystem, candidate: bool) -> Result<Validator, TestError> {
    let object = DiagnosticObject::Summary(incident()?);
    let historical = key(17)?;
    let original = filesystem.put(object, LEGACY);
    let copy = filesystem.put_stage(object, historical, LEGACY);
    let next = if candidate {
        filesystem.erase_existing(object);
        // Preserve the external original-copy while observing the actual migrated target.
        let copy = filesystem.put_stage(object, historical, LEGACY);
        let next = filesystem.put(object, CANDIDATE);
        filesystem.put(DiagnosticObject::Journal(historical), JOURNAL);
        return Ok(Validator::new(plan(historical, original, copy, next)?));
    } else {
        filesystem.put_stage(object, historical, CANDIDATE)
    };
    filesystem.put(DiagnosticObject::Journal(historical), JOURNAL);
    Ok(Validator::new(plan(historical, original, copy, next)?))
}
