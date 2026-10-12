// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire exact protected journals only after checked complete ledger settlement.
use super::*;
#[test]
fn diagnostic_recovery_exact_absent_original_and_candidate_retire_journal_once() -> TestResult {
    for case in [DiagnosticRecoveryLedgerCase::Original, DiagnosticRecoveryLedgerCase::Candidate] {
        admitted(move |context, _| {
            let filesystem = Filesystem::new();
            let (mut storage, operation) = setup(&context, filesystem.clone())?;
            let plan = recovery_plan(operation.key())?;
            filesystem.put(plan.victim, b"victim");
            filesystem.erase_existing(plan.victim);
            let current =
                if case == DiagnosticRecoveryLedgerCase::Original { ORIGINAL } else { CANDIDATE };
            let (mut journal, mut ledger) =
                reads(&mut storage, &filesystem, &operation, plan, current, &context)?;
            let mut validation = validator(plan, case);
            if case == DiagnosticRecoveryLedgerCase::Original {
                let stage_facts = filesystem.put_stage(plan.completion, plan.operation, CANDIDATE);
                validation.candidate_stage = Some(RecordedDiagnosticFileFacts::new(
                    stage_facts.file_identity(),
                    stage_facts.sha256(),
                    stage_facts.bytes(),
                )?);
                storage.inventory(&operation, &context)?;
            }
            let mut permit = storage.recover_disposition(
                &mut journal,
                &mut ledger,
                &validation,
                &operation,
                &context,
            )?;
            assert_eq!(storage.snapshot().reads(), 0);
            assert_eq!(storage.snapshot().live_handles(), 1);
            assert!(storage.prepare_delete(&mut permit, &operation, &context).is_err());
            let completion = if case == DiagnosticRecoveryLedgerCase::Original {
                let charge = storage
                    .shared
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .charged_bytes;
                let mut stage =
                    storage.recover_completion_stage(&mut permit, &operation, &context)?;
                assert_eq!(
                    storage
                        .shared
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .charged_bytes,
                    charge
                );
                assert_eq!(
                    stage
                        .write_chunk(CANDIDATE, &context)
                        .err()
                        .ok_or("recovered stage rewritable")?
                        .code(),
                    Code::Busy
                );
                stage.flush(&context)?;
                let previous = filesystem
                    .0
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .mutations;
                let active = storage.activate(
                    &mut stage,
                    DiagnosticPredecessor::Exact {
                        file_identity: ledger.facts().file_identity(),
                        sha256: ledger.facts().sha256(),
                    },
                    &operation,
                    &context,
                )?;
                assert_eq!(
                    filesystem
                        .0
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .mutations,
                    previous + 1
                );
                storage.inventory(&operation, &context)?;
                active
            } else {
                storage.reconcile_activation(
                    DiagnosticObject::StoreState,
                    DiagnosticPredecessor::Exact {
                        file_identity: ledger.facts().file_identity(),
                        sha256: ledger.facts().sha256(),
                    },
                    &operation,
                    &context,
                )?
            };
            let deleted =
                storage.retire_disposition(&mut permit, &completion, &operation, &context)?;
            assert_eq!(deleted.object(), DiagnosticObject::Journal(plan.operation));
            assert!(!storage.snapshot().disposition_retained());
            assert!(
                storage.retire_disposition(&mut permit, &completion, &operation, &context).is_err()
            );
            storage.retire(&operation, &context)?;
            Ok(())
        })?;
    }
    Ok(())
}
