// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Report each zoom's motion, so an intermittently missing zoom animation names its cause.

use super::*;

impl Session {
    /// Log the zoom the last input made: its wheel gap, chosen duration and why it jumped, if it
    /// did.
    pub(super) fn log_zoom(&mut self) {
        if let Some(zoom) = self.document.tools.take_zoom() {
            tracing::info!(
                operation = "canvas_zoom",
                gap_ms = zoom.gap.map(|gap| gap.as_secs_f64() * 1000.0),
                duration_ms = zoom.duration.as_secs_f64() * 1000.0,
                snapped = ?zoom.snapped,
                "A zoom moved the canvas camera"
            );
        }
    }
}
