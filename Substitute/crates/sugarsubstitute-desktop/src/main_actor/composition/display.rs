// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Report the refresh rate of the display showing the composed main window's canvas.
//!
//! The canvas paces navigation and presentation to the display it is shown on; without this
//! report it keeps the portable 60 Hz fallback on faster displays.

use crate::canvas::MainCanvasDisplay;
use tao::platform::windows::MonitorHandleExtWindows;

impl crate::main_actor::MainActor {
    /// Observe the showing display once per composition lifetime and again after each move to
    /// another display. Unavailable timing reports `None`, the portable fallback.
    pub(crate) fn take_canvas_display(&mut self) -> Option<crate::DesktopStartupObservation> {
        if !self.revealed || self.admission.closing() || !self.hosting().presents_native_canvas() {
            return None;
        }
        let monitor = self.window().ok()?.current_monitor()?;
        let showing = monitor.hmonitor();
        let composition = self.composition.as_mut()?;
        if composition.display == Some(showing) {
            return None;
        }
        composition.display = Some(showing);
        let hertz = crate::canvas::windows::display::refresh(&monitor).unwrap_or_else(|cause| {
            tracing::warn!(operation = "canvas_display_fallback", cause = %cause,
                "Active display timing unavailable; using 60 Hz");
            None
        });
        Some(crate::DesktopStartupObservation::CanvasDisplay(MainCanvasDisplay {
            identity: self.identity(),
            hertz,
        }))
    }
}
