// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Show complete native surfaces and acknowledge real presentation handoffs.
use crate::{DesktopError, WindowContent};
use sugarsubstitute_contracts::presentation::FrameIdentity;

/// Keep native visibility separate from eligibility to prepare the initial covered frame.
#[derive(Default)]
pub(crate) struct Visibility {
    prepared: bool,
    shown: bool,
    ever_shown: bool,
}

#[cfg(test)]
#[path = "runtime_visibility_tests.rs"]
mod tests;
impl Visibility {
    /// Admit the first complete frame while its native window is still covered.
    pub(crate) fn preparing_first(&self) -> bool {
        !self.prepared
    }
    /// Query actual successful native show/hide transitions owned by this lifetime.
    pub(crate) fn shown(&self) -> bool {
        self.shown
    }
    /// Record the first native show once even when a surface is later hidden and restored.
    pub(crate) fn ever_shown(&self) -> bool {
        self.ever_shown
    }
    /// Publish only after native pixels and all controls were submitted successfully.
    pub(crate) fn submitted(
        &mut self,
        content: &mut impl WindowContent,
        identity: FrameIdentity,
        native: impl FnMut(bool) -> Result<(), DesktopError>,
    ) -> Result<(), DesktopError> {
        self.prepared = true;
        self.reconcile(content, native)?;
        content
            .frame_presented(identity)
            .map_err(|cause| DesktopError::boxed("frame_presented", cause))
    }
    /// Apply requested visibility before acknowledging a hide to the application owner.
    pub(crate) fn reconcile(
        &mut self,
        content: &mut impl WindowContent,
        mut native: impl FnMut(bool) -> Result<(), DesktopError>,
    ) -> Result<(), DesktopError> {
        let requested = self.prepared && content.presentation_visible();
        if requested == self.shown {
            return Ok(());
        }
        native(requested)?;
        self.shown = requested;
        self.ever_shown |= requested;
        if !requested {
            content
                .presentation_hidden()
                .map_err(|cause| DesktopError::boxed("presentation_hidden", cause))?;
        }
        Ok(())
    }
}
