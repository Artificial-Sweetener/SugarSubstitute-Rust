// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual protected recovery transitions with a semantic adapter boundary fake.
mod clocks;
mod inventory;
mod settlement;
mod stage;
mod victim;
mod victim_context;
use super::*;
use sha2::{Digest, Sha256};
use std::sync::atomic::AtomicUsize;
const JOURNAL: &[u8] = b"journal-ledger";
const ORIGINAL: &[u8] = b"original-ledger";
const CANDIDATE: &[u8] = b"candidate-ledger";
fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
struct Validator {
    plan: DiagnosticDispositionPlan,
    case: DiagnosticRecoveryLedgerCase,
    calls: AtomicUsize,
    failure: bool,
    candidate_stage: Option<RecordedDiagnosticFileFacts>,
}
impl DiagnosticRecoveryValidator for Validator {
    fn validate(
        &self,
        current: LogicalStoreOperationKey,
        journal: &DiagnosticRecoverySource,
        ledger: &DiagnosticRecoverySource,
    ) -> Result<DiagnosticRecoveryDecision, DiagnosticRecoveryValidationError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.failure {
            return Err(DiagnosticRecoveryValidationError::caused(
                DiagnosticRecoveryValidationCode::InvalidSchema,
                std::io::Error::other("private decoder cause"),
            ));
        }
        if current.owner().namespace != self.plan.operation.owner().namespace
            || journal.as_bytes() != JOURNAL
            || ledger.as_bytes().is_empty()
        {
            return Err(DiagnosticRecoveryValidationError::rejected(
                DiagnosticRecoveryValidationCode::InvalidJournal,
            ));
        }
        let expected = self.candidate_stage.unwrap_or(
            if self.case == DiagnosticRecoveryLedgerCase::Candidate {
                RecordedDiagnosticFileFacts::new(
                    ledger.facts().file_identity(),
                    digest(CANDIDATE),
                    CANDIDATE.len() as u64,
                )
            } else {
                RecordedDiagnosticFileFacts::new([9; 24], digest(CANDIDATE), CANDIDATE.len() as u64)
            }
            .map_err(|cause| {
                DiagnosticRecoveryValidationError::caused(
                    DiagnosticRecoveryValidationCode::InvalidIdentity,
                    cause,
                )
            })?,
        );
        DiagnosticRecoveryDecision::new(
            self.plan,
            self.case,
            digest(ORIGINAL),
            digest(CANDIDATE),
            expected,
        )
    }
}
fn recovery_plan(owner: LogicalStoreOperationKey) -> Result<DiagnosticDispositionPlan, TestError> {
    let key = incident()?;
    Ok(DiagnosticDispositionPlan::from_recorded(
        DiagnosticObject::Summary(key),
        key,
        IncidentRevision::new(1)?,
        RecordedDiagnosticFileFacts::new([1; 24], digest(b"victim"), 6)?,
        owner,
        DispositionIntent::SummaryEviction,
        digest(JOURNAL),
        DiagnosticObject::StoreState,
        digest(CANDIDATE),
    )?)
}
fn validator(plan: DiagnosticDispositionPlan, case: DiagnosticRecoveryLedgerCase) -> Validator {
    Validator { plan, case, calls: AtomicUsize::new(0), failure: false, candidate_stage: None }
}
fn reads(
    storage: &mut DiagnosticStorage,
    filesystem: &Filesystem,
    operation: &DiagnosticStorageOperation,
    plan: DiagnosticDispositionPlan,
    ledger_bytes: &[u8],
    context: &JobContext,
) -> Result<(DiagnosticReadLease, DiagnosticReadLease), TestError> {
    filesystem.put(DiagnosticObject::Journal(plan.operation), JOURNAL);
    filesystem.put(DiagnosticObject::StoreState, ledger_bytes);
    Ok((
        storage.read(DiagnosticObject::Journal(plan.operation), 65536, operation, context)?,
        storage.read(DiagnosticObject::StoreState, 65536, operation, context)?,
    ))
}
#[test]
fn diagnostic_recovery_present_original_retains_exact_victim_and_historical_journal() -> TestResult
{
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let historical = LogicalStoreOperationKey::new(owner_key(3)?, IncidentOperationId::new(1)?);
        let plan = recovery_plan(historical)?;
        filesystem.put(plan.victim, b"victim");
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        assert_ne!(historical, operation.key());
        let (mut journal, mut ledger) =
            reads(&mut storage, &filesystem, &operation, plan, ORIGINAL, &context)?;
        let validation = validator(plan, DiagnosticRecoveryLedgerCase::Original);
        let mut permit = storage.recover_disposition(
            &mut journal,
            &mut ledger,
            &validation,
            &operation,
            &context,
        )?;
        assert_eq!(storage.snapshot().reads(), 0);
        assert_eq!(storage.snapshot().live_handles(), 2);
        assert!(journal.read_chunk(0, &mut [0; 1], &context).is_err());
        assert!(ledger.read_chunk(0, &mut [0; 1], &context).is_err());
        let mut victim = storage.prepare_delete(&mut permit, &operation, &context)?;
        assert_eq!(storage.snapshot().live_handles(), 2);
        assert!(storage.prepare_delete(&mut permit, &operation, &context).is_err());
        storage.delete(&mut victim, &mut permit, &operation, &context)?;
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_recovery_candidate_present_and_third_ledger_preserve_custody() -> TestResult {
    for third in [false, true] {
        admitted(move |context, _| {
            let filesystem = Filesystem::new();
            let (mut storage, operation) = setup(&context, filesystem.clone())?;
            let plan = recovery_plan(operation.key())?;
            if !third {
                filesystem.put(plan.victim, b"victim");
            }
            let (mut journal, mut ledger) = reads(
                &mut storage,
                &filesystem,
                &operation,
                plan,
                if third { b"third" } else { CANDIDATE },
                &context,
            )?;
            let error = storage
                .recover_disposition(
                    &mut journal,
                    &mut ledger,
                    &validator(plan, DiagnosticRecoveryLedgerCase::Candidate),
                    &operation,
                    &context,
                )
                .err()
                .ok_or("invalid recovery minted permit")?;
            assert_eq!(error.code(), Code::ChangedIdentity);
            assert_eq!(storage.snapshot().live_handles(), 2);
            assert_eq!(storage.snapshot().reads(), if third { 2 } else { 1 });
            assert_eq!(storage.snapshot().disposition_retained(), !third);
            assert_eq!(
                filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
                0
            );
            storage.retire(&operation, &context)?;
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn diagnostic_recovery_whole_read_hash_and_decoder_failures_preserve_original_sources() -> TestResult
{
    for corrupt in [false, true] {
        admitted(move |context, _| {
            let filesystem = Filesystem::new();
            let (mut storage, operation) = setup(&context, filesystem.clone())?;
            let plan = recovery_plan(operation.key())?;
            let (mut journal, mut ledger) =
                reads(&mut storage, &filesystem, &operation, plan, ORIGINAL, &context)?;
            let mut validation = validator(plan, DiagnosticRecoveryLedgerCase::Original);
            validation.failure = !corrupt;
            filesystem
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .faults
                .corrupt_read = corrupt;
            let error = storage
                .recover_disposition(&mut journal, &mut ledger, &validation, &operation, &context)
                .err()
                .ok_or("invalid read minted permit")?;
            assert_eq!(validation.calls.load(Ordering::SeqCst), usize::from(!corrupt));
            if corrupt {
                assert_eq!(error.code(), Code::ChangedIdentity);
            } else {
                assert!(std::error::Error::source(&error).is_some());
                assert!(!format!("{error:?}").contains("private decoder cause"));
            }
            assert_eq!(storage.snapshot().reads(), 2);
            assert!(!storage.snapshot().disposition_retained());
            storage.retire(&operation, &context)?;
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn diagnostic_recovery_failed_ledger_close_retains_actual_journal_victim_and_charge() -> TestResult
{
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let plan = recovery_plan(operation.key())?;
        filesystem.put(plan.victim, b"victim");
        let (mut journal, mut ledger) =
            reads(&mut storage, &filesystem, &operation, plan, ORIGINAL, &context)?;
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close = true;
        assert_eq!(
            storage
                .recover_disposition(
                    &mut journal,
                    &mut ledger,
                    &validator(plan, DiagnosticRecoveryLedgerCase::Original),
                    &operation,
                    &context
                )
                .err()
                .ok_or("failed ledger close minted permit")?
                .code(),
            Code::Retirement
        );
        assert_eq!(storage.snapshot().live_handles(), 3);
        assert_eq!(storage.snapshot().reads(), 1);
        assert!(storage.snapshot().disposition_retained());
        assert!(storage.retire(&operation, &context).is_err());
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close = false;
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
