// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain source evidence for explicitly audited successful native presentations.
//!
//! This UI-thread-local owner compares complete frames while clones share immutable buffers.
//! State retains the first frame and, only while different, the preceding frame.
//! It allocates no worker, scheduler or file; receipt ownership transfers after
//! the native surface has shut down.

use crate::{DesktopError, FrameAuditReceipt, MovePresentation};
use std::time::Duration;
use sugarsubstitute_contracts::presentation::PreparedFrame;

/// Bound retained move-loop evidence independently of raster dimensions or run duration.
const MOVE_SAMPLE_LIMIT: usize = 512;

/// Own at most two retained source frames throughout an optional native audit.
#[derive(Default)]
pub(crate) struct FrameAudit {
    first: Option<PreparedFrame>,
    previous: Option<PreparedFrame>,
    presented_frames: u64,
    content_changes: u64,
    move_presentations: Vec<MovePresentation>,
    move_frames_observed: u64,
    move_samples_omitted: u64,
}

impl FrameAudit {
    /// Consume only the result produced after native presentation and control painting.
    ///
    /// Presentation errors pass through unchanged. Failed native presentations
    /// leave counts and retained source allocations unchanged.
    /// The caller runs on the desktop UI thread and never submits before presentation.
    /// Supply elapsed time only when owned move-loop state was confirmed both
    /// immediately before submission and after successful presentation/control painting.
    /// Sample allocation errors preserve their cause and leave the audit unchanged.
    pub(crate) fn record(
        &mut self,
        presentation: Result<PreparedFrame, DesktopError>,
        move_elapsed: Option<Duration>,
    ) -> Result<(), DesktopError> {
        let frame = presentation?;
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
fn same_source(left: &PreparedFrame, right: &PreparedFrame) -> bool {
    left.width() == right.width()
        && left.height() == right.height()
        && left.pixels() == right.pixels()
}

#[cfg(test)]
#[path = "frame_audit_tests.rs"]
mod tests;
