// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain source evidence for explicitly audited successful native presentations.
//!
//! This UI-thread-local owner compares complete frames without cloning buffers.
//! State retains the first frame and, only while different, the preceding frame.
//! It allocates no worker, scheduler or file; receipt ownership transfers after
//! the native surface has shut down.

use crate::{DesktopError, Frame, FrameAuditReceipt};

/// Own at most two retained source frames throughout an optional native audit.
#[derive(Default)]
pub(crate) struct FrameAudit {
    first: Option<Frame>,
    previous: Option<Frame>,
    presented_frames: u64,
    content_changes: u64,
}

impl FrameAudit {
    /// Consume only the result produced after native presentation and control painting.
    ///
    /// Presentation errors pass through unchanged. Invalid frames and failed
    /// presentations leave counts and retained source allocations unchanged.
    /// The caller runs on the desktop UI thread and never submits before presentation.
    pub(crate) fn record(
        &mut self,
        presentation: Result<Frame, DesktopError>,
    ) -> Result<(), DesktopError> {
        let frame = presentation?;
        frame.validate()?;
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
        Ok(())
    }

    /// Transfer the original first frame and counts; an unpresented audit has no receipt.
    pub(crate) fn finish(self) -> Option<FrameAuditReceipt> {
        self.first.map(|first_frame| FrameAuditReceipt {
            first_frame,
            presented_frames: self.presented_frames,
            content_changes: self.content_changes,
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
        audit.record(Ok(first))?;
        audit.record(Ok(sample(10)))?;
        assert!(audit.previous.is_none());
        audit.record(Ok(sample(20)))?;
        assert!(audit.previous.is_some());
        audit.record(Ok(sample(20)))?;
        audit.record(Ok(sample(10)))?;
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
        audit.record(Ok(sample(10)))?;
        let mut dimensions = sample(10);
        dimensions.width = 1;
        dimensions.height = 2;
        audit.record(Ok(dimensions))?;
        audit.record(Ok(sample(10)))?;
        let mut alpha = sample(10);
        alpha.premultiplied_rgba[7] = 127;
        audit.record(Ok(alpha))?;
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
        assert!(audit.record(Ok(invalid)).is_err());
        let failure = DesktopError::message("present_fixture", "native presentation failed");
        assert_eq!(
            audit
                .record(Err(failure))
                .err()
                .ok_or("presentation failure was accepted")?
                .to_string(),
            "present_fixture: native presentation failed"
        );
        assert_eq!(audit.presented_frames, 0);
        assert_eq!(audit.content_changes, 0);
        assert!(audit.first.is_none());
        assert!(audit.previous.is_none());
        audit.record(Ok(sample(10)))?;
        let failure = DesktopError::message("controls_fixture", "native control painting failed");
        assert!(audit.record(Err(failure)).is_err());
        let receipt = audit.finish().ok_or("successful audit did not produce a receipt")?;
        assert_eq!(receipt.presented_frames, 1);
        assert_eq!(receipt.content_changes, 0);
        assert!(FrameAudit::default().finish().is_none());
        Ok(())
    }
}
