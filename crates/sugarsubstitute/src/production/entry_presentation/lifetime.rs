// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire shared Root rendering before allowing the entry execution owner to join.
use super::*;
impl<C: EntryActivation> EntryPresentation<C> {
    /// Fence presentation and request the existing Root lifetime shutdown.
    pub(super) fn close_owned(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Ok(());
        }
        self.closing = true;
        self.visible = false;
        let render =
            self.preparation.as_mut().map(PreparationDriver::request_retirement).transpose();
        let entry = self.entry_mut()?.request_shutdown();
        render?;
        entry
    }
    /// Deliver every pending shared batch, then retire render jobs before Root/global shutdown.
    pub(super) fn poll_owned_shutdown(&mut self) -> Result<bool, Box<dyn Error>> {
        if !self.closing {
            return Ok(false);
        }
        if let Some(render) = &mut self.preparation {
            self.entry
                .as_mut()
                .ok_or("Entry lifetime already transferred")?
                .consume_presentation(render, NonZeroUsize::MIN.saturating_add(31))?;
            if !render.poll_retirement()? {
                return Ok(false);
            }
        }
        self.entry_mut()?.poll_shutdown()
    }
}
