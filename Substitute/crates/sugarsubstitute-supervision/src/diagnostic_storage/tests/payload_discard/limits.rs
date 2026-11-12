// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit unrelated readers within retained native reservation while fencing selected files.
use super::*;
#[test]
fn diagnostic_payload_decision_retains_destination_until_actual_data_drop() -> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = setup(&context, disk()?)?;
        let (preparation, validator, original, candidate) =
            prepare(&mut storage, &operation, &context)?;
        let allocation = storage.payload_decision(&operation, &context)?;
        assert_eq!(allocation.victims.capacity(), MAX_ATTACHMENTS);
        let decision = validator.decision(allocation, DiagnosticPayloadDiscardCase::Original)?;
        assert_eq!(decision.allocated_victim_capacity(), MAX_ATTACHMENTS);
        storage.retire(&operation, &context)?;
        drop((preparation, validator, original, candidate, operation, storage));
        assert!(context.memory().claim(1).is_err());
        drop(decision);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_preflight_counts_unreturned_outputs_before_victim_effects()
-> TestResult {
    admitted(|context, _| {
        let disk = disk()?;
        let other = IncidentKey::new(IncidentSubject::Supervisor(owner_key(3)?));
        disk.put(DiagnosticObject::Seed(other), b"seed");
        let (mut storage, op) = setup(&context, disk.clone())?;
        let mut original =
            storage.read(DiagnosticObject::Summary(incident()?), 64, &op, &context)?;
        let mut candidate =
            storage.stage(DiagnosticObject::Summary(incident()?), 64, &op, &context)?;
        candidate.write_chunk(CANDIDATE, &context)?;
        candidate.flush(&context)?;
        let mut stages = Vec::new();
        for _ in 0..5 {
            stages.push(storage.stage(DiagnosticObject::Seed(other), 64, &op, &context)?);
        }
        disk.set_failure(DiagnosticFixtureFailure::PartialOpen, true);
        assert!(storage.read(DiagnosticObject::Seed(other), 64, &op, &context).is_err());
        disk.set_failure(DiagnosticFixtureFailure::PartialOpen, false);
        assert_eq!(disk.snapshot().live_outputs(), 8);
        let count = disk.snapshot().mutation_count();
        assert_eq!(
            storage
                .prepare_payload_discard(&mut original, &mut candidate, selection()?, &op, &context)
                .err()
                .map(|error| error.code()),
            Some(Code::Capacity)
        );
        assert_eq!(disk.snapshot().mutation_count(), count);
        assert!(!storage.snapshot().disposition_retained());
        storage.retire(&op, &context)?;
        assert_eq!(disk.snapshot().live_outputs(), 0);
        drop(stages);
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_selected_export_pin_prevents_preparation_before_effects() -> TestResult
{
    admitted(|context, _| {
        let disk = disk()?;
        let (mut storage, op) = setup(&context, disk.clone())?;
        let mut original =
            storage.read(DiagnosticObject::Summary(incident()?), 64, &op, &context)?;
        let mut export = storage.read(payload(0)?, 64, &op, &context)?;
        let mut candidate =
            storage.stage(DiagnosticObject::Summary(incident()?), 64, &op, &context)?;
        candidate.write_chunk(CANDIDATE, &context)?;
        candidate.flush(&context)?;
        let count = disk.snapshot().mutation_count();
        assert!(
            storage
                .prepare_payload_discard(&mut original, &mut candidate, selection()?, &op, &context)
                .is_err()
        );
        assert_eq!(disk.snapshot().mutation_count(), count);
        export.retire(&context)?;
        storage.retire(&op, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_unrelated_reads_preserve_four_slot_cap_and_selected_fence()
-> TestResult {
    admitted(|context, _| {
        let disk = disk()?;
        let other = IncidentKey::new(IncidentSubject::Supervisor(owner_key(3)?));
        disk.put(DiagnosticObject::Seed(other), b"seed");
        let (mut storage, op) = setup(&context, disk.clone())?;
        let (mut preparation, validator, _, _) = prepare(&mut storage, &op, &context)?;
        let mut reads = Vec::new();
        for _ in 0..3 {
            reads.push(storage.read(DiagnosticObject::Seed(other), 64, &op, &context)?);
        }
        assert_eq!(
            storage
                .read(DiagnosticObject::Seed(other), 64, &op, &context)
                .err()
                .map(|error| error.code()),
            Some(Code::Capacity)
        );
        assert_eq!(
            storage
                .read(DiagnosticObject::Summary(incident()?), 64, &op, &context)
                .err()
                .map(|error| error.code()),
            Some(Code::Busy)
        );
        assert_eq!(
            storage.read(payload(0)?, 64, &op, &context).err().map(|error| error.code()),
            Some(Code::Busy)
        );
        for read in &mut reads {
            read.retire(&context)?;
        }
        let mut stage =
            storage.seal_payload_discard(&mut preparation, JOURNAL, &validator, &op, &context)?;
        let mut permit =
            storage.activate_payload_discard(&mut preparation, &mut stage, &op, &context)?;
        let complete = storage.complete_payload_discard(&mut permit, &op, &context)?;
        storage.finish_payload_discard(&mut permit, &complete, &op, &context)?;
        storage.retire(&op, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_exhausted_revision_and_nonretention_reason_reject_data() -> TestResult
{
    let selected = selection()?;
    assert!(
        DiagnosticPayloadDiscardSelection::new(
            incident()?,
            IncidentRevision::new(u64::MAX)?,
            IncidentRevision::new(1)?,
            selected.attachments().clone(),
            selected.disposition()
        )
        .is_err()
    );
    assert!(
        DiagnosticPayloadDiscardSelection::new(
            incident()?,
            IncidentRevision::new(1)?,
            IncidentRevision::new(1)?,
            selected.attachments().clone(),
            DiscardedPayload::new(
                PayloadDiscard::UserRequested,
                selected.attachments().total_bytes()
            )?
        )
        .is_err()
    );
    Ok(())
}
