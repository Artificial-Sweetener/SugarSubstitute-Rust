// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve native partial outputs and role fences through callback reentry.
use super::*;
#[test]
fn diagnostic_payload_discard_changed_candidate_after_seal_blocks_journal_and_summary_effects()
-> TestResult {
    admitted(|context, _| {
        let disk = disk()?;
        let (mut storage, op) = setup(&context, disk.clone())?;
        let (mut preparation, validator, _, _) = prepare(&mut storage, &op, &context)?;
        let mut stage =
            storage.seal_payload_discard(&mut preparation, JOURNAL, &validator, &op, &context)?;
        disk.change_stage(1);
        let count = disk.snapshot().mutation_count();
        assert_eq!(
            storage
                .activate_payload_discard(&mut preparation, &mut stage, &op, &context)
                .err()
                .map(|error| error.code()),
            Some(Code::ChangedIdentity)
        );
        assert_eq!(disk.snapshot().mutation_count(), count);
        assert!(disk.snapshot().active_identity(DiagnosticObject::Journal(op.key)).is_none());
        storage.retire(&op, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_partial_journal_write_and_failed_flush_never_admit_activation()
-> TestResult {
    for failure in [DiagnosticFixtureFailure::PartialWrite, DiagnosticFixtureFailure::Flush] {
        admitted(move |context, _| {
            let disk = disk()?;
            let (mut storage, op) = setup(&context, disk.clone())?;
            let (mut preparation, validator, _, _) = prepare(&mut storage, &op, &context)?;
            disk.set_failure(failure, true);
            assert!(
                storage
                    .seal_payload_discard(&mut preparation, JOURNAL, &validator, &op, &context)
                    .is_err()
            );
            assert_eq!(disk.snapshot().live_outputs(), 5);
            assert_eq!(
                disk.snapshot()
                    .active_identity(DiagnosticObject::Summary(incident()?))
                    .map(|facts| facts.sha256()),
                Some(Sha256::digest(ORIGINAL).into())
            );
            for index in 0..2 {
                assert!(disk.snapshot().active_identity(payload(index)?).is_some());
            }
            disk.set_failure(failure, false);
            storage.retire(&op, &context)?;
            assert_eq!(disk.snapshot().live_outputs(), 0);
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn diagnostic_payload_discard_callback_changed_candidate_facts_blocks_journal_effects() -> TestResult
{
    admitted(|context, _| {
        let disk = disk()?;
        let (mut storage, op) = setup(&context, disk.clone())?;
        let (mut preparation, validator, _, _) = prepare(&mut storage, &op, &context)?;
        let changed = disk.clone();
        *validator.hook.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(Box::new(move || changed.change_stage(1)));
        let count = disk.snapshot().mutation_count();
        assert_eq!(
            storage
                .seal_payload_discard(&mut preparation, JOURNAL, &validator, &op, &context)
                .err()
                .map(|error| error.code()),
            Some(Code::ChangedIdentity)
        );
        assert_eq!(disk.snapshot().mutation_count(), count);
        storage.retire(&op, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_callback_reentry_is_fenced_without_holding_owner_mutex() -> TestResult
{
    admitted(|context, _| {
        let disk = disk()?;
        let (mut storage, op) = setup(&context, disk.clone())?;
        let (mut preparation, validator, original, mut candidate) =
            prepare(&mut storage, &op, &context)?;
        let original = Arc::new(Mutex::new(original));
        let borrow = original.clone();
        let callback_context = context.clone();
        *validator.hook.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(Box::new(move || {
                assert_eq!(
                    borrow
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .retire(&callback_context)
                        .err()
                        .map(|error| error.code()),
                    Some(Code::Busy)
                )
            }));
        let mut journal =
            storage.seal_payload_discard(&mut preparation, JOURNAL, &validator, &op, &context)?;
        assert_eq!(
            candidate.write_chunk(b"changed", &context).err().map(|error| error.code()),
            Some(Code::Busy)
        );
        assert!(storage.inventory(&op, &context).is_err());
        assert!(storage.read(payload(0)?, 64, &op, &context).is_err());
        let mut permit =
            storage.activate_payload_discard(&mut preparation, &mut journal, &op, &context)?;
        let complete = storage.complete_payload_discard(&mut permit, &op, &context)?;
        storage.finish_payload_discard(&mut permit, &complete, &op, &context)?;
        storage.retire(&op, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_partial_journal_create_retains_output_and_charge() -> TestResult {
    admitted(|context, _| {
        let disk = disk()?;
        let (mut storage, op) = setup(&context, disk.clone())?;
        let (mut preparation, validator, _, _) = prepare(&mut storage, &op, &context)?;
        disk.set_failure(DiagnosticFixtureFailure::PartialCreate, true);
        assert!(
            storage
                .seal_payload_discard(&mut preparation, JOURNAL, &validator, &op, &context)
                .is_err()
        );
        assert_eq!(disk.snapshot().live_outputs(), 5);
        assert!(storage.snapshot().disposition_retained());
        assert!(context.memory().claim(1).is_err());
        disk.set_failure(DiagnosticFixtureFailure::PartialCreate, false);
        storage.retire(&op, &context)?;
        assert_eq!(disk.snapshot().live_outputs(), 0);
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_partial_victim_open_retains_unreturned_native_output() -> TestResult {
    admitted(|context, _| {
        let disk = disk()?;
        let (mut storage, op) = setup(&context, disk.clone())?;
        let mut original =
            storage.read(DiagnosticObject::Summary(incident()?), 64, &op, &context)?;
        let mut candidate =
            storage.stage(DiagnosticObject::Summary(incident()?), 64, &op, &context)?;
        candidate.write_chunk(CANDIDATE, &context)?;
        candidate.flush(&context)?;
        disk.set_failure(DiagnosticFixtureFailure::PartialOpen, true);
        let count = disk.snapshot().mutation_count();
        assert!(
            storage
                .prepare_payload_discard(&mut original, &mut candidate, selection()?, &op, &context)
                .is_err()
        );
        assert_eq!(disk.snapshot().live_outputs(), 3);
        assert_eq!(disk.snapshot().mutation_count(), count);
        disk.set_failure(DiagnosticFixtureFailure::PartialOpen, false);
        storage.retire(&op, &context)?;
        assert_eq!(disk.snapshot().live_outputs(), 0);
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_journal_rename_retry_cannot_recreate_or_change_sealed_bytes()
-> TestResult {
    admitted(|context, _| {
        let disk = disk()?;
        let (mut storage, op) = setup(&context, disk.clone())?;
        let (mut preparation, validator, _, _) = prepare(&mut storage, &op, &context)?;
        let mut stage =
            storage.seal_payload_discard(&mut preparation, JOURNAL, &validator, &op, &context)?;
        disk.set_failure(DiagnosticFixtureFailure::PostActivation, true);
        assert!(
            storage.activate_payload_discard(&mut preparation, &mut stage, &op, &context).is_err()
        );
        let count = disk.snapshot().mutation_count();
        assert!(
            storage
                .seal_payload_discard(&mut preparation, b"changed", &validator, &op, &context)
                .is_err()
        );
        disk.set_failure(DiagnosticFixtureFailure::PostActivation, false);
        let mut permit =
            storage.activate_payload_discard(&mut preparation, &mut stage, &op, &context)?;
        assert_eq!(disk.snapshot().mutation_count(), count);
        let complete = storage.complete_payload_discard(&mut permit, &op, &context)?;
        storage.finish_payload_discard(&mut permit, &complete, &op, &context)?;
        storage.retire(&op, &context)?;
        Ok(())
    })
}
