// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve exact historical candidate custody across interrupted recovery.
use super::*;
pub(super) fn prepared(
    context: &JobContext,
    filesystem: &Filesystem,
    present: bool,
) -> Result<
    (
        DiagnosticStorage,
        DiagnosticStorageOperation,
        DurableDeletionPermit,
        DiagnosticDispositionPlan,
        DiagnosticReadFacts,
        DiagnosticReadFacts,
    ),
    TestError,
> {
    let historical = LogicalStoreOperationKey::new(owner_key(3)?, IncidentOperationId::new(1)?);
    let plan = recovery_plan(historical)?;
    filesystem.put(plan.victim, b"victim");
    if !present {
        filesystem.erase_existing(plan.victim);
    }
    let candidate = filesystem.put_stage(plan.completion, historical, CANDIDATE);
    let (mut storage, operation) = setup(context, filesystem.clone())?;
    let (mut journal, mut ledger) =
        reads(&mut storage, filesystem, &operation, plan, ORIGINAL, context)?;
    let original = ledger.facts();
    let mut validation = validator(plan, DiagnosticRecoveryLedgerCase::Original);
    validation.candidate_stage = Some(RecordedDiagnosticFileFacts::new(
        candidate.file_identity(),
        candidate.sha256(),
        candidate.bytes(),
    )?);
    let permit =
        storage.recover_disposition(&mut journal, &mut ledger, &validation, &operation, context)?;
    Ok((storage, operation, permit, plan, original, candidate))
}
#[test]
fn diagnostic_recovery_death_after_candidate_activation_reconciles_same_inode_without_reapplication()
-> TestResult {
    for uncertain in [false, true] {
        admitted(move |context, _| {
            let filesystem = Filesystem::new();
            let (mut storage, operation, mut permit, plan, original, candidate) =
                prepared(&context, &filesystem, false)?;
            let before =
                filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations;
            let mut stage = storage.recover_completion_stage(&mut permit, &operation, &context)?;
            assert_eq!(
                storage.shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner).stages[0]
                    .reserved,
                0
            );
            filesystem
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .faults
                .activate = uncertain;
            let before_wrong =
                filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations;
            assert_eq!(
                storage
                    .activate(&mut stage, DiagnosticPredecessor::Absent, &operation, &context)
                    .err()
                    .ok_or("changed recovery predecessor accepted")?
                    .code(),
                Code::InvalidIdentity
            );
            assert_eq!(
                filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
                before_wrong
            );
            assert_eq!(storage.snapshot().stages(), 1);
            let result = storage.activate(
                &mut stage,
                DiagnosticPredecessor::Exact {
                    file_identity: original.file_identity(),
                    sha256: original.sha256(),
                },
                &operation,
                &context,
            );
            let active = if uncertain {
                assert_eq!(
                    result.err().ok_or("uncertain activation yielded proof")?.code(),
                    Code::ActivationUncertain
                );
                assert_eq!(storage.snapshot().live_handles(), 2);
                assert_eq!(storage.snapshot().stages(), 1);
                None
            } else {
                let active = result?;
                assert_eq!(active.facts(), candidate);
                Some(active)
            };
            assert_eq!(
                filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
                before + 1
            );
            storage.retire(&operation, &context)?;
            drop(active);
            drop(stage);
            drop(permit);
            drop(operation);
            drop(storage);
            let memory = context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?;
            let allocation = allocation::owner(memory)?;
            let owned = filesystem.retain_claim(&allocation)?;
            let mut storage =
                DiagnosticStorage::from_port(allocation, OwnedPort::new(Box::new(owned), None))?;
            let operation = storage.begin_operation(key(2)?, &context)?;
            storage.resolve(
                Path::new("ignored-by-external-fake"),
                operation.key().owner().namespace,
                &operation,
                &context,
            )?;
            storage.acquire_writer(&operation, &context)?;
            storage.inventory(&operation, &context)?;
            let mut journal = storage.read(
                DiagnosticObject::Journal(plan.operation),
                65536,
                &operation,
                &context,
            )?;
            let mut ledger = storage.read(plan.completion, 65536, &operation, &context)?;
            let mut validation = validator(plan, DiagnosticRecoveryLedgerCase::Candidate);
            validation.candidate_stage = Some(RecordedDiagnosticFileFacts::new(
                candidate.file_identity(),
                candidate.sha256(),
                candidate.bytes(),
            )?);
            let mut permit = storage.recover_disposition(
                &mut journal,
                &mut ledger,
                &validation,
                &operation,
                &context,
            )?;
            assert_eq!(
                storage
                    .recover_completion_stage(&mut permit, &operation, &context)
                    .err()
                    .ok_or("Candidate reactivated")?
                    .code(),
                Code::Busy
            );
            let completion = storage.reconcile_activation(
                plan.completion,
                DiagnosticPredecessor::Exact {
                    file_identity: candidate.file_identity(),
                    sha256: candidate.sha256(),
                },
                &operation,
                &context,
            )?;
            storage.retire_disposition(&mut permit, &completion, &operation, &context)?;
            assert_eq!(
                filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
                before + 2
            );
            storage.retire(&operation, &context)?;
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn diagnostic_recovery_stage_partial_open_and_failed_close_retain_original_charge() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation, mut permit, _, _, _) = prepared(&context, &filesystem, false)?;
        filesystem
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .faults
            .partial_open = true;
        let before =
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations;
        assert!(storage.recover_completion_stage(&mut permit, &operation, &context).is_err());
        assert_eq!(storage.snapshot().stages(), 1);
        assert_eq!(storage.snapshot().live_handles(), 2);
        assert_eq!(
            storage.shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner).stages[0]
                .reserved,
            0
        );
        assert_eq!(
            storage
                .recover_completion_stage(&mut permit, &operation, &context)
                .err()
                .ok_or("duplicate recovery admitted")?
                .code(),
            Code::Busy
        );
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close = true;
        assert!(storage.retire(&operation, &context).is_err());
        assert_eq!(storage.snapshot().live_handles(), 2);
        assert_eq!(
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
            before
        );
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close = false;
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_recovery_candidate_same_bytes_different_inode_never_mints_authority() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let plan = recovery_plan(operation.key())?;
        let (mut journal, mut ledger) =
            reads(&mut storage, &filesystem, &operation, plan, CANDIDATE, &context)?;
        let mut validation = validator(plan, DiagnosticRecoveryLedgerCase::Candidate);
        validation.candidate_stage = Some(RecordedDiagnosticFileFacts::new(
            [9; 24],
            digest(CANDIDATE),
            CANDIDATE.len() as u64,
        )?);
        assert_eq!(
            storage
                .recover_disposition(&mut journal, &mut ledger, &validation, &operation, &context)
                .err()
                .ok_or("changed candidate inode accepted")?
                .code(),
            Code::ChangedIdentity
        );
        assert_eq!(storage.snapshot().reads(), 2);
        assert_eq!(storage.snapshot().live_handles(), 2);
        assert!(!storage.snapshot().disposition_retained());
        storage.retire(&operation, &context)?;
        Ok(())
    })
}

#[test]
fn diagnostic_recovery_stage_requires_fresh_inventory_after_actual_victim_delete() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation, mut permit, _, _, _) = prepared(&context, &filesystem, true)?;
        assert_eq!(
            storage
                .recover_completion_stage(&mut permit, &operation, &context)
                .err()
                .ok_or("unsettled victim accepted")?
                .code(),
            Code::Busy
        );
        let mut victim = storage.prepare_delete(&mut permit, &operation, &context)?;
        storage.delete(&mut victim, &mut permit, &operation, &context)?;
        assert_eq!(
            storage
                .recover_completion_stage(&mut permit, &operation, &context)
                .err()
                .ok_or("stale inventory accepted")?
                .code(),
            Code::InventoryIncomplete
        );
        storage.inventory(&operation, &context)?;
        let mut stage = storage.recover_completion_stage(&mut permit, &operation, &context)?;
        stage.retire(&context)?;
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_recovery_changed_or_missing_stage_preserves_journal_and_adopted_outputs() -> TestResult
{
    for fault in 0..5 {
        admitted(move |context, _| {
            let filesystem = Filesystem::new();
            let (mut storage, operation, mut permit, _, _, _) =
                prepared(&context, &filesystem, false)?;
            filesystem.change_stage(fault);
            let before =
                filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations;
            assert!(storage.recover_completion_stage(&mut permit, &operation, &context).is_err());
            assert_eq!(
                storage.snapshot().live_handles(),
                if matches!(fault, 1..=3) { 2 } else { 1 }
            );
            assert!(storage.snapshot().disposition_retained());
            assert_eq!(storage.snapshot().stages(), 1);
            assert_eq!(
                filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
                before
            );
            storage.retire(&operation, &context)?;
            Ok(())
        })?;
    }
    Ok(())
}
