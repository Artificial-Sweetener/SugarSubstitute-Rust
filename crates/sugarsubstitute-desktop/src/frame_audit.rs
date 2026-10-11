// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain source evidence for explicitly audited successful native presentations.
//!
//! This UI-thread-local owner compares complete frames without cloning buffers.
//! State retains the first frame and, only while different, the preceding frame.
//! It allocates no worker, scheduler or file; receipt ownership transfers after
//! the native surface has shut down.

use crate::{DesktopError, Frame, FrameAuditReceipt, MovePresentation};
use std::time::Duration;

/// Bound retained move-loop evidence independently of raster dimensions or run duration.
const MOVE_SAMPLE_LIMIT: usize = 512;

/// Own at most two retained source frames throughout an optional native audit.
#[derive(Default)]
pub(crate) struct FrameAudit {
    first: Option<Frame>,
    previous: Option<Frame>,
    presented_frames: u64,
    content_changes: u64,
    move_presentations: Vec<MovePresentation>,
    move_frames_observed: u64,
    move_samples_omitted: u64,
}

impl FrameAudit {
    /// Consume only the result produced after native presentation and control painting.
    ///
    /// Presentation errors pass through unchanged. Invalid frames and failed
    /// presentations leave counts and retained source allocations unchanged.
    /// The caller runs on the desktop UI thread and never submits before presentation.
    /// Supply elapsed time only when owned move-loop state was confirmed both
    /// immediately before rendering and after successful presentation/control painting.
    /// Sample allocation errors preserve their cause and leave the audit unchanged.
    pub(crate) fn record(
        &mut self,
        presentation: Result<Frame, DesktopError>,
        move_elapsed: Option<Duration>,
    ) -> Result<(), DesktopError> {
        let frame = presentation?;
        frame.validate()?;
        if move_elapsed.is_some()
            && self.move_presentations.len() < MOVE_SAMPLE_LIMIT
            && self.move_presentations.capacity() < MOVE_SAMPLE_LIMIT
        {
            self.move_presentations
                .try_reserve_exact(MOVE_SAMPLE_LIMIT - self.move_presentations.len())
                .map_err(|error| DesktopError::new("reserve_native_move_audit", error))?;
        }
        if let Some(first) = &self.first {
            let previous = self.previous.as_ref().unwrap_or(first);
            if !same_source(previous, &frame) {
                self.content_changes += 1;
                self.previous = (!same_source(first, &frame)).then_some(frame);
            }
        } else {
            self.first = Some(frame);
        }
        self.presented_frames += 1;
        if let Some(elapsed) = move_elapsed {
            self.move_frames_observed += 1;
            if self.move_presentations.len() < MOVE_SAMPLE_LIMIT {
                self.move_presentations.push(MovePresentation {
                    sequence: self.presented_frames,
                    elapsed,
                    source_revision: self.content_changes,
                });
            } else {
                self.move_samples_omitted += 1;
            }
        }
        Ok(())
    }

    /// Transfer the original first frame and counts; an unpresented audit has no receipt.
    pub(crate) fn finish(self) -> Option<FrameAuditReceipt> {
        self.first.map(|first_frame| FrameAuditReceipt {
            first_frame,
            presented_frames: self.presented_frames,
            content_changes: self.content_changes,
            move_presentations: self.move_presentations,
            move_frames_observed: self.move_frames_observed,
            move_samples_omitted: self.move_samples_omitted,
        })
    }
}

/// Compare device dimensions and every source byte, including alpha.
fn same_source(left: &Frame, right: &Frame) -> bool {
    left.width == right.width
        && left.height == right.height
        && left.premultiplied_rgba == right.premultiplied_rgba
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    /// Create a valid two-pixel source with distinguishable color and alpha.
    fn sample(value: u8) -> Frame {
        Frame { width: 2, height: 1, premultiplied_rgba: vec![value, 0, 0, 255, 0, 0, 0, 128] }
    }

    /// Count both directions of a transient change and retain the first source without copying.
    #[test]
    fn whole_frame_audit_counts_change_then_restoration() -> Result<(), Box<dyn Error>> {
        let mut audit = FrameAudit::default();
        let first = sample(10);
        let original_allocation = first.premultiplied_rgba.as_ptr();
        audit.record(Ok(first), None)?;
        audit.record(Ok(sample(10)), None)?;
        assert!(audit.previous.is_none());
        audit.record(Ok(sample(20)), None)?;
        assert!(audit.previous.is_some());
        audit.record(Ok(sample(20)), None)?;
        audit.record(Ok(sample(10)), None)?;
        assert!(audit.previous.is_none());
        let receipt = audit.finish().ok_or("successful audit did not produce a receipt")?;
        assert_eq!(receipt.presented_frames, 5);
        assert_eq!(receipt.content_changes, 2);
        assert_eq!(receipt.first_frame.premultiplied_rgba.as_ptr(), original_allocation);
        assert_eq!(receipt.first_frame.premultiplied_rgba, sample(10).premultiplied_rgba);
        Ok(())
    }

    /// Compare shape and the last alpha byte even when every other source byte matches.
    #[test]
    fn whole_frame_audit_detects_dimensions_and_complete_pixel_content()
    -> Result<(), Box<dyn Error>> {
        let mut audit = FrameAudit::default();
        audit.record(Ok(sample(10)), None)?;
        let mut dimensions = sample(10);
        dimensions.width = 1;
        dimensions.height = 2;
        audit.record(Ok(dimensions), None)?;
        audit.record(Ok(sample(10)), None)?;
        let mut alpha = sample(10);
        alpha.premultiplied_rgba[7] = 127;
        audit.record(Ok(alpha), None)?;
        let receipt = audit.finish().ok_or("successful audit did not produce a receipt")?;
        assert_eq!(receipt.presented_frames, 4);
        assert_eq!(receipt.content_changes, 3);
        Ok(())
    }

    /// Reject malformed sources and failed presentations before mutating any audit state.
    #[test]
    fn whole_frame_audit_rejects_invalid_or_unsuccessful_presentations()
    -> Result<(), Box<dyn Error>> {
        let mut audit = FrameAudit::default();
        let mut invalid = sample(10);
        invalid.premultiplied_rgba.pop();
        assert!(audit.record(Ok(invalid), Some(Duration::from_millis(1))).is_err());
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
        audit.record(Ok(sample(10)), None)?;
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
        audit.record(Ok(sample(10)), None)?;
        audit.record(Ok(sample(20)), None)?;
        audit.record(Ok(sample(20)), Some(Duration::from_millis(10)))?;
        audit.record(Ok(sample(20)), Some(Duration::from_millis(20)))?;
        audit.record(Ok(sample(30)), None)?;
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
    fn move_audit_records_full_source_revisions_and_submission_times() -> Result<(), Box<dyn Error>>
    {
        let mut audit = FrameAudit::default();
        audit.record(Ok(sample(10)), None)?;
        for (value, elapsed) in [(20, 10), (30, 20), (10, 30), (10, 40)] {
            audit.record(Ok(sample(value)), Some(Duration::from_millis(elapsed)))?;
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
    fn move_audit_bounds_samples_and_reports_every_omitted_observation()
    -> Result<(), Box<dyn Error>> {
        let mut audit = FrameAudit::default();
        audit.record(Ok(sample(10)), None)?;
        for index in 0..515 {
            let value = if index % 2 == 0 { 20 } else { 30 };
            audit.record(Ok(sample(value)), Some(Duration::from_millis(index + 1)))?;
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
}
