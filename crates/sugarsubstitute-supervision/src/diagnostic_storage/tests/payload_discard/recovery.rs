// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recover exact historical intent phases through the same storage boundary.
use super::*;
#[test]
fn diagnostic_payload_discard_original_recovery_adopts_one_existing_candidate() -> TestResult {
    admitted(|context, _| {
        let (disk, validator) = recovered_disk(false, None)?;
        let (mut storage, op) = setup(&context, disk.clone())?;
        let count = disk.snapshot().mutation_count();
        let mut permit = recover(&mut storage, &op, &context, &validator)?;
        assert_eq!(disk.snapshot().mutation_count(), count);
        let complete = storage.complete_payload_discard(&mut permit, &op, &context)?;
        assert_eq!(complete.summary().facts().file_identity(), validator.candidate.file_identity());
        assert_eq!(disk.snapshot().mutation_count(), count + 3);
        storage.finish_payload_discard(&mut permit, &complete, &op, &context)?;
        storage.retire(&op, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_candidate_recovery_accepts_exact_present_and_absent_victims()
-> TestResult {
    admitted(|context, _| {
        let (disk, validator) = recovered_disk(true, Some(0))?;
        let (mut storage, op) = setup(&context, disk.clone())?;
        let count = disk.snapshot().mutation_count();
        let mut permit = recover(&mut storage, &op, &context, &validator)?;
        assert_eq!(storage.snapshot().stages(), 0);
        let complete = storage.complete_payload_discard(&mut permit, &op, &context)?;
        assert_eq!(disk.snapshot().mutation_count(), count + 1);
        assert_eq!(complete.summary().facts().file_identity(), validator.candidate.file_identity());
        storage.finish_payload_discard(&mut permit, &complete, &op, &context)?;
        storage.retire(&op, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_original_missing_victim_preserves_candidate_and_journal() -> TestResult
{
    admitted(|context, _| {
        let (disk, validator) = recovered_disk(false, Some(1))?;
        let (mut storage, op) = setup(&context, disk.clone())?;
        let count = disk.snapshot().mutation_count();
        assert!(recover(&mut storage, &op, &context, &validator).is_err());
        assert_eq!(disk.snapshot().mutation_count(), count);
        assert!(storage.snapshot().disposition_retained());
        assert!(
            disk.snapshot()
                .active_identity(DiagnosticObject::Journal(validator.historical))
                .is_some()
        );
        storage.retire(&op, &context)?;
        assert_eq!(disk.snapshot().live_outputs(), 0);
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_recovery_same_bytes_changed_victim_inode_is_blocked() -> TestResult {
    admitted(|context, _| {
        let (disk, validator) = recovered_disk(true, None)?;
        disk.erase_existing(payload(1)?);
        disk.put(payload(1)?, PAYLOADS[1]);
        let (mut storage, op) = setup(&context, disk.clone())?;
        let count = disk.snapshot().mutation_count();
        assert!(recover(&mut storage, &op, &context, &validator).is_err());
        assert_eq!(disk.snapshot().mutation_count(), count);
        assert!(disk.snapshot().active_identity(payload(0)?).is_some());
        storage.retire(&op, &context)?;
        Ok(())
    })
}
#[test]
fn diagnostic_payload_discard_recovery_changed_current_summary_blocks_before_victims() -> TestResult
{
    admitted(|context, _| {
        let (disk, validator) = recovered_disk(true, None)?;
        disk.erase_existing(DiagnosticObject::Summary(incident()?));
        disk.put(DiagnosticObject::Summary(incident()?), CANDIDATE);
        let (mut storage, op) = setup(&context, disk.clone())?;
        let count = disk.snapshot().mutation_count();
        assert!(recover(&mut storage, &op, &context, &validator).is_err());
        assert_eq!(disk.snapshot().mutation_count(), count);
        assert_eq!(disk.snapshot().live_outputs(), 2);
        storage.retire(&op, &context)?;
        Ok(())
    })
}
