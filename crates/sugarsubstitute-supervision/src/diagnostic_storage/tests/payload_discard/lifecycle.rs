// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize candidate-before-delete ordering and replay-safe exact settlement.
use super::*;
#[test]
fn diagnostic_payload_discard_preserves_original_arrival_order_and_finishes_exact_group()
-> TestResult {
    admitted(|context, _| {
        let disk = disk()?;
        let (mut storage, op) = setup(&context, disk.clone())?;
        let mut permit = normal(&mut storage, &op, &context)?;
        assert_eq!(
            selection()?.attachments().entries().map(|fact| fact.kind()).collect::<Vec<_>>(),
            KINDS
        );
        assert!(storage.snapshot().disposition_retained());
        assert_eq!(
            disk.snapshot()
                .active_identity(DiagnosticObject::Summary(incident()?))
                .map(|facts| facts.sha256()),
            Some(Sha256::digest(ORIGINAL).into())
        );
        for index in 0..2 {
            assert!(disk.snapshot().active_identity(payload(index)?).is_some());
        }
        let complete = storage.complete_payload_discard(&mut permit, &op, &context)?;
        assert_eq!(complete.reclaimed_bytes(), selection()?.attachments().total_bytes());
        assert_eq!(complete.reclaimed_payload_groups(), 1);
        for index in 0..2 {
            assert!(disk.snapshot().active_identity(payload(index)?).is_none());
        }
        let mutations = disk.snapshot().mutation_count();
        let repeated = storage.complete_payload_discard(&mut permit, &op, &context)?;
        assert_eq!(repeated.summary().facts(), complete.summary().facts());
        assert_eq!(disk.snapshot().mutation_count(), mutations);
        let deletion = storage.finish_payload_discard(&mut permit, &complete, &op, &context)?;
        assert_eq!(deletion.object(), DiagnosticObject::Journal(op.key));
        assert!(!storage.snapshot().disposition_retained());
        assert_eq!(disk.snapshot().live_outputs(), 0);
        storage.inventory(&op, &context)?;
        storage.finish_operation(&op, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_uncertain_candidate_retry_never_restages() -> TestResult {
    admitted(|context, _| {
        let disk = disk()?;
        let (mut storage, op) = setup(&context, disk.clone())?;
        let mut permit = normal(&mut storage, &op, &context)?;
        disk.set_failure(DiagnosticFixtureFailure::PostActivation, true);
        assert!(storage.complete_payload_discard(&mut permit, &op, &context).is_err());
        for index in 0..2 {
            assert!(disk.snapshot().active_identity(payload(index)?).is_some());
        }
        let count = disk.snapshot().mutation_count();
        disk.set_failure(DiagnosticFixtureFailure::PostActivation, false);
        let complete = storage.complete_payload_discard(&mut permit, &op, &context)?;
        assert_eq!(disk.snapshot().mutation_count(), count + 2);
        storage.finish_payload_discard(&mut permit, &complete, &op, &context)?;
        storage.retire(&op, &context)?;
        assert_eq!(disk.snapshot().live_outputs(), 0);
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_second_victim_failure_preserves_first_settled_state() -> TestResult {
    admitted(|context, _| {
        let disk = disk()?;
        let (mut storage, op) = setup(&context, disk.clone())?;
        let mut permit = normal(&mut storage, &op, &context)?;
        disk.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.delete_object =
            Some(payload(1)?);
        assert!(storage.complete_payload_discard(&mut permit, &op, &context).is_err());
        assert!(disk.snapshot().active_identity(payload(0)?).is_none());
        assert!(disk.snapshot().active_identity(payload(1)?).is_some());
        let mutations = disk.snapshot().mutation_count();
        disk.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.delete_object =
            None;
        let complete = storage.complete_payload_discard(&mut permit, &op, &context)?;
        assert_eq!(disk.snapshot().mutation_count(), mutations + 1);
        storage.finish_payload_discard(&mut permit, &complete, &op, &context)?;
        storage.retire(&op, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_failed_close_and_absence_resume_without_reopening() -> TestResult {
    admitted(|context, _| {
        let disk = disk()?;
        let (mut storage, op) = setup(&context, disk.clone())?;
        let mut permit = normal(&mut storage, &op, &context)?;
        disk.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close_object =
            Some(payload(0)?);
        assert!(storage.complete_payload_discard(&mut permit, &op, &context).is_err());
        assert_eq!(disk.snapshot().live_outputs(), 3);
        disk.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close_object = None;
        disk.set_failure(DiagnosticFixtureFailure::AbsenceObservation, true);
        assert!(storage.complete_payload_discard(&mut permit, &op, &context).is_err());
        assert_eq!(disk.snapshot().live_outputs(), 2);
        let mutations = disk.snapshot().mutation_count();
        disk.set_failure(DiagnosticFixtureFailure::AbsenceObservation, false);
        let complete = storage.complete_payload_discard(&mut permit, &op, &context)?;
        assert_eq!(disk.snapshot().mutation_count(), mutations + 1);
        storage.finish_payload_discard(&mut permit, &complete, &op, &context)?;
        storage.retire(&op, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_journal_close_and_absence_retry_keeps_completion() -> TestResult {
    admitted(|context, _| {
        let disk = disk()?;
        let (mut storage, op) = setup(&context, disk.clone())?;
        let mut permit = normal(&mut storage, &op, &context)?;
        let complete = storage.complete_payload_discard(&mut permit, &op, &context)?;
        disk.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close_object =
            Some(DiagnosticObject::Journal(op.key));
        assert!(storage.finish_payload_discard(&mut permit, &complete, &op, &context).is_err());
        disk.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close_object = None;
        disk.set_failure(DiagnosticFixtureFailure::AbsenceObservation, true);
        assert!(storage.finish_payload_discard(&mut permit, &complete, &op, &context).is_err());
        assert_eq!(disk.snapshot().live_outputs(), 0);
        assert!(storage.snapshot().disposition_retained());
        let count = disk.snapshot().mutation_count();
        disk.set_failure(DiagnosticFixtureFailure::AbsenceObservation, false);
        storage.finish_payload_discard(&mut permit, &complete, &op, &context)?;
        assert_eq!(disk.snapshot().mutation_count(), count);
        storage.retire(&op, &context)?;
        Ok(())
    })
}
