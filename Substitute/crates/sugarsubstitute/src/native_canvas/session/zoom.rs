// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Report each zoom's motion, so an intermittently missing zoom animation names its cause.

use super::*;

impl Session {
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
}
