// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Report each zoom's motion and what each camera motion presented, so a choppy or missing zoom
//! animation names its cause.

use super::*;

/// Why one display attempt during a camera motion submitted no frame.
#[derive(Clone, Copy)]
pub(super) enum Gate {
    Displaying,
    GpuPending,
    CpuPending,
    Pacer,
    NoBacking,
    Unchanged,
    Buffers,
}

/// A camera motion being measured: whether detail work ran when it began, and how often each
/// gate stopped a frame.
pub(super) struct MotionWatch {
    detail: bool,
    blocked: [u64; 7],
}

impl Session {
    /// Count a display attempt that a gate stopped while a camera motion is measured.
    pub(super) fn note_blocked(&mut self, gate: Gate) {
        if let Some(watch) = &mut self.motion {
            let count = &mut watch.blocked[gate as usize];
            *count = count.saturating_add(1);
        }
    }

    /// Record the zoom the last input made: its wheel gap, chosen duration and why it jumped, if
    /// it did. The desktop child writes it as a closed record the parent relays to the app log.
    pub(super) fn log_zoom(&mut self) {
        let Some(zoom) = self.document.tools.take_zoom() else { return };
        #[cfg(all(windows, any(feature = "native-composition", feature = "windows-native-host")))]
        crate::canvas_zoom_diagnostic::Record::from(zoom).emit();
        #[cfg(not(all(
            windows,
            any(feature = "native-composition", feature = "windows-native-host")
        )))]
        let _ = zoom;
    }

    /// Measure each camera motion: open a presentation window when the camera starts to move,
    /// and record what it presented once it settles and no frame is still in flight.
    pub(super) fn observe_motion(&mut self) -> Result<(), Box<dyn Error>> {
        let now = self.navigation_time();
        let moving = self.document.tools.navigation().resampling();
        match self.motion {
            None if moving => {
                let detail = self
                    .rendering
                    .as_ref()
                    .is_some_and(|rendering| rendering.tier == BackingTier::Detail);
                self.motion = Some(MotionWatch { detail, blocked: [0; 7] });
                self.pacing.begin_window(now);
            }
            Some(MotionWatch { detail, blocked }) if !moving => {
                if matches!(
                    self.pacing.readiness(now)?,
                    sugarsubstitute_canvas_render::PresentationReadiness::Busy { .. }
                ) {
                    return Ok(());
                }
                self.motion = None;
                let Some(window) = self.pacing.end_window() else { return Ok(()) };
                #[cfg(all(
                    windows,
                    any(feature = "native-composition", feature = "windows-native-host")
                ))]
                crate::canvas_motion_diagnostic::Record::new(
                    window,
                    now,
                    self.pacing.period(),
                    detail,
                    blocked,
                )
                .emit();
                #[cfg(not(all(
                    windows,
                    any(feature = "native-composition", feature = "windows-native-host")
                )))]
                let _ = (window, detail, blocked);
            }
            _ => {}
        }
        Ok(())
    }
}
