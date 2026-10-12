// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual recovered custody without inferring settlement from a busy operation.
use super::*;
use DiagnosticRecoveredVictimState::{Retained, Settled};

#[test]
fn diagnostic_recovered_victim_observation_tracks_issued_lease_and_checked_settlement() -> TestResult
{
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let plan = recovery_plan(key(1)?)?;
        filesystem.put(plan.victim, b"victim");
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let (mut journal, mut ledger) =
            reads(&mut storage, &filesystem, &operation, plan, ORIGINAL, &context)?;
        let mut permit = storage.recover_disposition(
            &mut journal,
            &mut ledger,
            &validator(plan, DiagnosticRecoveryLedgerCase::Original),
            &operation,
            &context,
        )?;
        assert_eq!(storage.recovered_victim_state(&mut permit, &operation, &context)?, Retained);
        let mut victim = storage.prepare_delete(&mut permit, &operation, &context)?;
        assert_eq!(storage.recovered_victim_state(&mut permit, &operation, &context)?, Retained);
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close = true;
        assert!(storage.delete(&mut victim, &mut permit, &operation, &context).is_err());
        assert_eq!(storage.recovered_victim_state(&mut permit, &operation, &context)?, Retained);
        assert_eq!(storage.snapshot().live_handles(), 2);
        {
            let mut disk = filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            disk.faults.close = false;
            disk.faults.absence = true;
        }
        assert!(storage.delete(&mut victim, &mut permit, &operation, &context).is_err());
        assert!(storage.recovered_victim_state(&mut permit, &operation, &context).is_err());
        assert_eq!(storage.snapshot().live_handles(), 1);
        assert!(storage.snapshot().disposition_retained());
        filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.absence =
            false;
        storage.delete(&mut victim, &mut permit, &operation, &context)?;
        let mutations =
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations;
        assert_eq!(storage.recovered_victim_state(&mut permit, &operation, &context)?, Settled);
        assert_eq!(
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
            mutations
        );
        storage.retire(&operation, &context)?;
        Ok(())
    })
}

#[test]
fn diagnostic_recovered_victim_observation_requires_actual_journal_and_exact_absence() -> TestResult
{
    for case in [DiagnosticRecoveryLedgerCase::Original, DiagnosticRecoveryLedgerCase::Candidate] {
        admitted(move |context, _| {
            let filesystem = Filesystem::new();
            let (mut storage, operation) = setup(&context, filesystem.clone())?;
            let plan = recovery_plan(operation.key())?;
            let bytes =
                if case == DiagnosticRecoveryLedgerCase::Original { ORIGINAL } else { CANDIDATE };
            let (mut journal, mut ledger) =
                reads(&mut storage, &filesystem, &operation, plan, bytes, &context)?;
            let mut permit = storage.recover_disposition(
                &mut journal,
                &mut ledger,
                &validator(plan, case),
                &operation,
                &context,
            )?;
            assert_eq!(storage.recovered_victim_state(&mut permit, &operation, &context)?, Settled);
            assert_eq!(storage.snapshot().live_handles(), 1);
            assert_eq!(
                filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).mutations,
                0
            );
            filesystem.erase_existing(DiagnosticObject::Journal(plan.operation));
            assert_eq!(
                storage
                    .recovered_victim_state(&mut permit, &operation, &context)
                    .err()
                    .ok_or("changed journal reported settled")?
                    .code(),
                Code::ChangedIdentity
            );
            assert!(storage.snapshot().disposition_retained());
            storage.retire(&operation, &context)?;
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn diagnostic_recovered_victim_observation_rejects_ordinary_permit() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let plan = recovery_plan(operation.key())?;
        let mut stage = storage.stage_disposition(plan, &operation, &context)?;
        stage.write_chunk(JOURNAL, &context)?;
        stage.flush(&context)?;
        let mut permit = storage.activate_disposition(
            &mut stage,
            DiagnosticPredecessor::Absent,
            &operation,
            &context,
        )?;
        assert_eq!(
            storage
                .recovered_victim_state(&mut permit, &operation, &context)
                .err()
                .ok_or("ordinary permit classified")?
                .code(),
            Code::InvalidIdentity
        );
        assert_eq!(storage.snapshot().live_handles(), 1);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}
