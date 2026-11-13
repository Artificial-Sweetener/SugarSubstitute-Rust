// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify actual successful-submission identity and timing accounting without native UI races.

use super::*;
use std::error::Error;
use sugarsubstitute_contracts::execution::{Generation, SurfaceId};
use sugarsubstitute_execution::qualification_time::anchor;

fn identity(state: u64, sequence: u64) -> Result<FrameIdentity, Box<dyn Error>> {
    Ok(FrameIdentity {
        surface: SurfaceId::new(1)?,
        state_generation: Generation::new(state)?,
        sequence: Generation::new(sequence)?,
    })
}

#[test]
fn successful_identity_changes_and_retained_exposes_have_parallel_exact_counts()
-> Result<(), Box<dyn Error>> {
    let start = anchor();
    let mut presentations = Presentations::new(start, false);
    for (index, identity) in
        [identity(1, 1)?, identity(1, 1)?, identity(1, 2)?, identity(1, 2)?, identity(2, 1)?]
            .into_iter()
            .enumerate()
    {
        let submitted = start + Duration::from_millis((index as u64 + 1) * 10);
        presentations.record_submission(
            Ok(()),
            identity,
            submitted - Duration::from_millis(1),
            submitted,
        )?;
    }
    let receipt = presentations.finish(NativeMaterial::ThemedFallback)?;
    assert_eq!(receipt.frames_presented, 5);
    assert_eq!(receipt.prepared_frames_presented, 3);
    assert_eq!(receipt.retained_frame_exposes, 2);
    assert_eq!(
        receipt.frames_presented,
        receipt.prepared_frames_presented + receipt.retained_frame_exposes
    );
    assert_eq!(receipt.first_frame, Duration::from_millis(10));
    assert_eq!(receipt.total_frame_work, Duration::from_millis(5));
    assert_eq!(receipt.total_frame_intervals, Duration::from_millis(40));
    assert_eq!(receipt.longest_frame_interval, Duration::from_millis(10));
    Ok(())
}

#[test]
fn failed_submission_does_not_consume_identity_or_timing_or_first_frame()
-> Result<(), Box<dyn Error>> {
    let start = anchor();
    let mut presentations = Presentations::new(start, false);
    let first = identity(1, 1)?;
    let failed = identity(1, 2)?;
    let failure = DesktopError::new("native_submit", std::io::Error::other("original cause"));
    let error = presentations
        .record_submission(Err(failure), first, start, start)
        .err()
        .ok_or("Failed native submission was counted")?;
    assert_eq!(error.source().ok_or("Cause lost")?.to_string(), "original cause");
    assert_eq!(presentations.first, None);
    assert_eq!(presentations.last_identity, None);
    presentations.record_submission(Ok(()), first, start, start + Duration::from_millis(10))?;
    let failure = DesktopError::message("native_controls", "control painting failed");
    assert!(
        presentations
            .record_submission(Err(failure), failed, start, start + Duration::from_millis(20))
            .is_err()
    );
    presentations.record_submission(
        Ok(()),
        first,
        start + Duration::from_millis(29),
        start + Duration::from_millis(30),
    )?;
    let receipt = presentations.finish(NativeMaterial::ThemedFallback)?;
    assert_eq!(receipt.frames_presented, 2);
    assert_eq!(receipt.prepared_frames_presented, 1);
    assert_eq!(receipt.retained_frame_exposes, 1);
    assert_eq!(receipt.first_frame, Duration::from_millis(10));
    assert_eq!(receipt.total_frame_work, Duration::from_millis(11));
    assert_eq!(receipt.total_frame_intervals, Duration::from_millis(20));
    Ok(())
}
