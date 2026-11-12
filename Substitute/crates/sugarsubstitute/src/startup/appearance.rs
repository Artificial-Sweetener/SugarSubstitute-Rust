// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the operating-system accent reported to the startup host and present it everywhere.

use super::*;
use sugarsubstitute_contracts::appearance::AccentColor;
use sugarsubstitute_ui::accent::AccentPalette;

/// Latest operating-system accent reported by the desktop; absent until reported.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SystemAccent(Option<AccentColor>);

impl SystemAccent {
    /// Palette for splash views created from now on; the product palette without an accent.
    pub(super) fn palette(self) -> AccentPalette {
        AccentPalette::resolve(self.0)
    }
}

impl<B: StartupBackend> StartupHost<B> {
    /// Adopt a reported accent, repaint the live splash and present it to SugarCubes' owner.
    /// `None` withdraws the system accent so both surfaces return to the product palette.
    /// # Errors
    /// Returns the child owner's failure to deliver the accent or to refresh its deadline.
    pub(super) fn adopt_system_accent(
        &mut self,
        accent: Option<AccentColor>,
    ) -> Result<(), Box<dyn Error>> {
        self.accent = SystemAccent(accent);
        let palette = self.accent.palette();
        let changed = self.view.as_mut().is_some_and(|view| view.set_accent(palette));
        self.invalidate(changed);
        self.child.system_accent(accent)?;
        self.refresh_deadline()
    }
}
