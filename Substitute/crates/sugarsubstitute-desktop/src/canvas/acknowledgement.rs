// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide main-window canvas acknowledgements independently of the platform compositor.
//!
//! The compositor adapter performs the native effects these decisions name; application tests
//! drive the same rules with a fake compositor so acknowledgement ordering is never re-derived.

use super::placement::CanvasPlacement;
use crate::DesktopError;
use sugarsubstitute_canvas::FrameStamp;

/// Native effect required to present one prepared frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameEffect {
    /// These exact pixels and placement are already committed. Only a frame still pending its
    /// single upload acknowledgement carries a stamp; a settled frame needs no acknowledgement.
    Committed(Option<FrameStamp>),
    /// Displaced surfaces remain in compositor custody; their completion wake retries this frame.
    AwaitRetirement,
    /// Upload and commit, then record success with [`Acknowledgement::committed`].
    Commit,
}

/// Native effect required to withdraw the committed canvas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HideEffect {
    /// Nothing is shown and no displaced surface remains, so the hide is complete.
    Hidden,
    /// Displaced surfaces remain in compositor custody; their completion wake retries the hide.
    AwaitRetirement,
    /// Disconnect the committed surface, then record success with
    /// [`Acknowledgement::hide_committed`]. Completion is observed on a later turn.
    Commit,
}

/// The last committed frame and placement of one retained native canvas visual.
#[derive(Debug, Default)]
pub struct Acknowledgement {
    last: Option<(FrameStamp, CanvasPlacement)>,
}

impl Acknowledgement {
    /// Choose the effect for a prepared frame. `retiring` reports whether any displaced surface
    /// still awaits compositor completion.
    /// # Errors
    /// Rejects a frame older than the committed frame of the same surface.
    pub fn frame(
        &self,
        target: FrameStamp,
        placement: CanvasPlacement,
        pending: bool,
        retiring: bool,
    ) -> Result<FrameEffect, DesktopError> {
        if self.last.is_some_and(|(current, _)| {
            current.surface == target.surface && current.generation > target.generation
        }) {
            return Err(DesktopError::message("canvas_frame", "frame generation regressed"));
        }
        if self.last == Some((target, placement)) {
            return Ok(FrameEffect::Committed(pending.then_some(target)));
        }
        if retiring {
            return Ok(FrameEffect::AwaitRetirement);
        }
        Ok(FrameEffect::Commit)
    }

    /// Record a frame whose native commit succeeded.
    pub fn committed(&mut self, target: FrameStamp, placement: CanvasPlacement) {
        self.last = Some((target, placement));
    }

    /// Choose the effect for withdrawing the canvas; `retiring` is as for [`Self::frame`].
    pub fn hide(&self, retiring: bool) -> HideEffect {
        match (self.last, retiring) {
            (Some(_), _) => HideEffect::Commit,
            (None, true) => HideEffect::AwaitRetirement,
            (None, false) => HideEffect::Hidden,
        }
    }

    /// Record a hide whose native commit succeeded.
    pub fn hide_committed(&mut self) {
        self.last = None;
    }

    /// Whether some committed native frame currently occupies exactly this placement.
    pub fn shows(&self, placement: CanvasPlacement) -> bool {
        self.last.is_some_and(|(_, shown)| shown == placement)
    }

    /// Whether this exact frame and placement are the committed native content.
    pub fn retains(&self, target: FrameStamp, placement: CanvasPlacement) -> bool {
        self.last == Some((target, placement))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::placement::PhysicalRect;
    use sugarsubstitute_contracts::execution::{Generation, SurfaceId};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn stamp(generation: u64) -> Result<FrameStamp, Box<dyn std::error::Error>> {
        Ok(FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(generation)? })
    }

    fn placement(width: u32) -> CanvasPlacement {
        let rect = PhysicalRect { x: 0, y: 0, width, height: 40 };
        CanvasPlacement { viewport: rect, clip: rect }
    }

    #[test]
    fn a_committed_frame_acknowledges_only_its_pending_upload() -> TestResult {
        let mut ack = Acknowledgement::default();
        assert_eq!(ack.frame(stamp(1)?, placement(80), true, false)?, FrameEffect::Commit);
        ack.committed(stamp(1)?, placement(80));
        let pending = ack.frame(stamp(1)?, placement(80), true, true)?;
        assert_eq!(pending, FrameEffect::Committed(Some(stamp(1)?)));
        let settled = ack.frame(stamp(1)?, placement(80), false, true)?;
        assert_eq!(settled, FrameEffect::Committed(None));
        assert!(ack.retains(stamp(1)?, placement(80)));
        assert!(!ack.retains(stamp(1)?, placement(60)));
        Ok(())
    }

    #[test]
    fn a_new_frame_waits_for_displaced_surfaces_and_rejects_regression() -> TestResult {
        let mut ack = Acknowledgement::default();
        ack.committed(stamp(2)?, placement(80));
        let moved = ack.frame(stamp(2)?, placement(60), false, true)?;
        assert_eq!(moved, FrameEffect::AwaitRetirement);
        assert_eq!(ack.frame(stamp(3)?, placement(80), true, false)?, FrameEffect::Commit);
        assert!(ack.frame(stamp(1)?, placement(80), true, false).is_err());
        Ok(())
    }

    #[test]
    fn a_hide_completes_only_after_its_commit_and_retirement() -> TestResult {
        let mut ack = Acknowledgement::default();
        assert_eq!(ack.hide(false), HideEffect::Hidden);
        ack.committed(stamp(1)?, placement(80));
        assert_eq!(ack.hide(false), HideEffect::Commit);
        ack.hide_committed();
        assert!(!ack.retains(stamp(1)?, placement(80)));
        assert_eq!(ack.hide(true), HideEffect::AwaitRetirement);
        assert_eq!(ack.hide(false), HideEffect::Hidden);
        Ok(())
    }
}
