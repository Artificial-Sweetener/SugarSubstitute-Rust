// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the Main caption's window buttons laid out, and answer what the application asks
//! about this window's standing.
use super::*;

impl MainActor {
    /// Whether this window is closing or has retired.
    pub(crate) fn closing(&self) -> bool {
        self.admission.closing() || self.retired
    }

    /// Keep this window from being destroyed while a window it owns is open, because Windows
    /// destroys owned windows with their owner, behind their own owner's back.
    pub(crate) fn hold_for_owned_window(&mut self, open: bool) {
        self.owned_window_open = open;
    }

    /// Lay the window buttons out for the current client size without moving the browser.
    pub(super) fn resize_caption(&mut self) -> Result<(), DesktopError> {
        let controls = self.caption_controls()?;
        if let Some(owner) = &mut self.controls {
            owner.update(&crate::windows_controls::Specification::buttons(&controls))?;
        }
        Ok(())
    }
}
