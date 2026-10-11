// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify opt-in full-source evidence using real prepared frame validation.

use super::*;
use std::{error::Error, num::NonZeroUsize};
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryLease, MemoryRelease, SurfaceId},
    presentation::{FrameIdentity, FrameShape, FrameValidator},
};

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

/// Prepare immutable test storage through the real alpha validator and lease claim.
fn prepared(width: u32, height: u32, pixels: Vec<u8>) -> Result<PreparedFrame, Box<dyn Error>> {
    let shape = FrameShape::new(width, height, 1.0)?;
    let identity = FrameIdentity {
        surface: SurfaceId::new(1)?,
        state_generation: Generation::new(1)?,
        sequence: Generation::new(1)?,
    };
    let lease = MemoryLease::new(pixels.capacity() as u64, Box::new(Release));
    let mut validator = FrameValidator::new(shape, identity, pixels, lease)?;
    if !validator.validate_next(NonZeroUsize::new(2).ok_or("Nonzero budget")?)? {
        return Err("Fixture validation incomplete".into());
    }
    Ok(validator.finish()?)
}

/// Create a valid two-pixel source with distinguishable color and alpha.
fn sample(value: u8) -> Result<PreparedFrame, Box<dyn Error>> {
    prepared(2, 1, vec![value, 0, 0, 255, 0, 0, 0, 128])
}

/// Count both directions of a transient change and retain the first source without copying.
#[test]
fn whole_frame_audit_counts_change_then_restoration() -> Result<(), Box<dyn Error>> {
    let mut audit = FrameAudit::default();
    let first = sample(10)?;
    let original_allocation = first.pixels().as_ptr();
    audit.record(Ok(first), None)?;
    audit.record(Ok(sample(10)?), None)?;
    assert!(audit.previous.is_none());
    audit.record(Ok(sample(20)?), None)?;
    assert!(audit.previous.is_some());
    audit.record(Ok(sample(20)?), None)?;
    audit.record(Ok(sample(10)?), None)?;
    assert!(audit.previous.is_none());
    let receipt = audit.finish().ok_or("successful audit did not produce a receipt")?;
    assert_eq!(receipt.presented_frames, 5);
    assert_eq!(receipt.content_changes, 2);
    assert_eq!(receipt.first_frame.pixels().as_ptr(), original_allocation);
    assert_eq!(receipt.first_frame.pixels(), sample(10)?.pixels());
    Ok(())
}

/// Audit and receipt clones retain one actual allocation/charge until their final owner drops.
#[test]
fn audit_retains_shared_prepared_allocation_and_charge_until_receipt_drop()
-> Result<(), Box<dyn Error>> {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct CountRelease(Arc<AtomicUsize>);
    impl MemoryRelease for CountRelease {
        fn release(&self, _: u64) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let released = Arc::new(AtomicUsize::new(0));
    let original = sample(10)?;
    let pixels = original.pixels().to_vec();
    let lease =
        MemoryLease::new(pixels.capacity() as u64, Box::new(CountRelease(released.clone())));
    let mut validator = FrameValidator::new(original.shape(), original.identity(), pixels, lease)?;
    assert!(validator.validate_next(NonZeroUsize::new(2).ok_or("Nonzero budget")?)?);
    let frame = validator.finish()?;
    let allocation = frame.pixels().as_ptr();
    let mut audit = FrameAudit::default();
    audit.record(Ok(frame.clone()), None)?;
    assert_eq!(audit.first.as_ref().ok_or("Audit lost source")?.pixels().as_ptr(), allocation);
    drop(frame);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    let receipt = audit.finish().ok_or("Audit lost receipt")?;
    assert_eq!(receipt.first_frame.pixels().as_ptr(), allocation);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(receipt);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    Ok(())
}

/// Compare shape and the last alpha byte even when every other source byte matches.
#[test]
fn whole_frame_audit_detects_dimensions_and_complete_pixel_content() -> Result<(), Box<dyn Error>> {
    let mut audit = FrameAudit::default();
    audit.record(Ok(sample(10)?), None)?;
    let dimensions = prepared(1, 2, sample(10)?.pixels().to_vec())?;
    audit.record(Ok(dimensions), None)?;
    audit.record(Ok(sample(10)?), None)?;
    let alpha = prepared(2, 1, vec![10, 0, 0, 255, 0, 0, 0, 127])?;
    audit.record(Ok(alpha), None)?;
    let receipt = audit.finish().ok_or("successful audit did not produce a receipt")?;
    assert_eq!(receipt.presented_frames, 4);
    assert_eq!(receipt.content_changes, 3);
    Ok(())
}

/// Preserve counts and retained source ownership when native presentation fails.
#[test]
fn whole_frame_audit_preserves_state_after_unsuccessful_presentations() -> Result<(), Box<dyn Error>>
{
    let mut audit = FrameAudit::default();
    let failure = DesktopError::message("present_fixture", "native presentation failed");
    assert_eq!(
        audit
            .record(Err(failure), Some(Duration::from_millis(2)))
            .err()
            .ok_or("presentation failure was accepted")?
            .to_string(),
        "present_fixture: native presentation failed"
    );
    assert_eq!(audit.presented_frames, 0);
    assert_eq!(audit.content_changes, 0);
    assert!(audit.first.is_none());
    assert!(audit.previous.is_none());
    assert!(audit.move_presentations.is_empty());
    assert_eq!(audit.move_frames_observed, 0);
    assert_eq!(audit.move_samples_omitted, 0);
    assert_eq!(audit.move_presentations.capacity(), 0);
    audit.record(Ok(sample(10)?), None)?;
    let failure = DesktopError::message("controls_fixture", "native control painting failed");
    assert!(audit.record(Err(failure), Some(Duration::from_millis(3))).is_err());
    let receipt = audit.finish().ok_or("successful audit did not produce a receipt")?;
    assert_eq!(receipt.presented_frames, 1);
    assert_eq!(receipt.content_changes, 0);
    assert!(receipt.move_presentations.is_empty());
    assert_eq!(receipt.move_frames_observed, 0);
    assert!(FrameAudit::default().finish().is_none());
    Ok(())
}

/// Global changes before and after a move do not imply a transition between its submissions.
#[test]
fn move_audit_preserves_global_sequence_without_claiming_outside_transitions()
-> Result<(), Box<dyn Error>> {
    let mut audit = FrameAudit::default();
    audit.record(Ok(sample(10)?), None)?;
    audit.record(Ok(sample(20)?), None)?;
    audit.record(Ok(sample(20)?), Some(Duration::from_millis(10)))?;
    audit.record(Ok(sample(20)?), Some(Duration::from_millis(20)))?;
    audit.record(Ok(sample(30)?), None)?;
    let receipt = audit.finish().ok_or("successful move audit did not produce a receipt")?;
    assert_eq!(receipt.presented_frames, 5);
    assert_eq!(receipt.content_changes, 2);
    assert_eq!(receipt.move_frames_observed, 2);
    assert_eq!(receipt.move_samples_omitted, 0);
    assert_eq!(
        receipt.move_presentations,
        vec![
            MovePresentation {
                sequence: 3,
                elapsed: Duration::from_millis(10),
                source_revision: 1
            },
            MovePresentation {
                sequence: 4,
                elapsed: Duration::from_millis(20),
                source_revision: 1
            },
        ]
    );
    Ok(())
}

/// Record changing source submissions and restoration with their actual observation times.
#[test]
fn move_audit_records_full_source_revisions_and_submission_times() -> Result<(), Box<dyn Error>> {
    let mut audit = FrameAudit::default();
    audit.record(Ok(sample(10)?), None)?;
    for (value, elapsed) in [(20, 10), (30, 20), (10, 30), (10, 40)] {
        audit.record(Ok(sample(value)?), Some(Duration::from_millis(elapsed)))?;
    }
    assert!(audit.previous.is_none());
    let receipt = audit.finish().ok_or("successful move audit did not produce a receipt")?;
    assert_eq!(receipt.content_changes, 3);
    assert_eq!(receipt.move_frames_observed, 4);
    assert_eq!(
        receipt.move_presentations,
        vec![
            MovePresentation {
                sequence: 2,
                elapsed: Duration::from_millis(10),
                source_revision: 1
            },
            MovePresentation {
                sequence: 3,
                elapsed: Duration::from_millis(20),
                source_revision: 2
            },
            MovePresentation {
                sequence: 4,
                elapsed: Duration::from_millis(30),
                source_revision: 3
            },
            MovePresentation {
                sequence: 5,
                elapsed: Duration::from_millis(40),
                source_revision: 3
            },
        ]
    );
    Ok(())
}

/// Continue observing complete source transitions after the exact retained-sample cap.
#[test]
fn move_audit_bounds_samples_and_reports_every_omitted_observation() -> Result<(), Box<dyn Error>> {
    let mut audit = FrameAudit::default();
    audit.record(Ok(sample(10)?), None)?;
    for index in 0..515 {
        let value = if index % 2 == 0 { 20 } else { 30 };
        audit.record(Ok(sample(value)?), Some(Duration::from_millis(index + 1)))?;
    }
    assert_eq!(audit.move_presentations.capacity(), MOVE_SAMPLE_LIMIT);
    let receipt = audit.finish().ok_or("successful move audit did not produce a receipt")?;
    assert_eq!(receipt.presented_frames, 516);
    assert_eq!(receipt.content_changes, 515);
    assert_eq!(receipt.move_frames_observed, 515);
    assert_eq!(receipt.move_samples_omitted, 3);
    assert_eq!(receipt.move_presentations.len(), MOVE_SAMPLE_LIMIT);
    assert_eq!(
        receipt.move_presentations.first(),
        Some(&MovePresentation {
            sequence: 2,
            elapsed: Duration::from_millis(1),
            source_revision: 1,
        })
    );
    assert_eq!(
        receipt.move_presentations.last(),
        Some(&MovePresentation {
            sequence: 513,
            elapsed: Duration::from_millis(512),
            source_revision: 512,
        })
    );
    Ok(())
}
