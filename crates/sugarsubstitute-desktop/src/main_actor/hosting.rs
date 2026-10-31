// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide composition or windowed browser hosting once per main lifetime.

use crate::{DesktopError, MainHosting};

/// Track the single hosting decision made before the main WebView is constructed.
///
/// Composition is attempted first. When its native surface cannot be created, the partial
/// visual tree retires before windowed hosting is selected, so an uncommitted or half-bound
/// tree never overlays the windowed browser. A selected hosting never changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HostingSelection {
    /// Composition surface effects are still being attempted.
    Probing,
    /// Composition failed; its partial native resources are retiring.
    Retiring,
    /// Browser hosting is fixed for this lifetime.
    Selected(MainHosting),
}

impl HostingSelection {
    /// Begin by attempting composition hosting.
    pub(super) const fn new() -> Self {
        Self::Probing
    }

    /// Fix composition hosting after its complete native surface exists.
    pub(super) fn surface_ready(&mut self) -> Result<(), DesktopError> {
        self.advance(Self::Probing, Self::Selected(MainHosting::Composition), "select_composition")
    }

    /// Begin retiring a composition surface whose native creation failed.
    pub(super) fn surface_failed(&mut self) -> Result<(), DesktopError> {
        self.advance(Self::Probing, Self::Retiring, "reject_composition")
    }

    /// Fix windowed hosting only after the failed surface has actually retired.
    pub(super) fn partial_retired(&mut self) -> Result<(), DesktopError> {
        self.advance(Self::Retiring, Self::Selected(MainHosting::Windowed), "select_windowed")
    }

    /// Return the fixed hosting, or nothing while the decision is still pending.
    pub(super) const fn selected(self) -> Option<MainHosting> {
        match self {
            Self::Selected(hosting) => Some(hosting),
            Self::Probing | Self::Retiring => None,
        }
    }

    fn advance(
        &mut self,
        expected: Self,
        next: Self,
        operation: &'static str,
    ) -> Result<(), DesktopError> {
        if *self != expected {
            return Err(DesktopError::message(operation, "hosting decision is out of order"));
        }
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
#[path = "hosting_tests.rs"]
mod tests;
