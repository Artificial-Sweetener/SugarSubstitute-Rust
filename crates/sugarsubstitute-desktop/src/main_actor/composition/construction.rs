// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Install the owned composition tree before the admitted frontend navigates.

use super::*;

impl MainActor {
    /// Advance browser composition and attach its native input only after commit.
    pub(in crate::main_actor) fn prepare_composition(&mut self) -> Result<bool, DesktopError> {
        if self.composition.is_none() {
            let claim = self
                .request()?
                .storage_lease()
                .claim(65536)
                .map_err(|cause| DesktopError::new("claim_browser_composition", cause))?;
            self.composition =
                Some(Composition::new(self.window()?.clone(), self.view()?.controller(), claim));
        }
        let ready =
            self.composition.as_mut().ok_or_else(|| missing("browser_composition"))?.advance()?;
        if ready {
            let events = self.events()?.clone();
            self.composition
                .as_mut()
                .ok_or_else(|| missing("browser_composition"))?
                .prepare_input(events)?;
        }
        Ok(ready)
    }
}
