// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide composition or windowed browser hosting once per main lifetime.

use crate::{DesktopError, MainHosting};

/// Track the single hosting decision, which is fixed before the parent observes a controller.
///
/// Composition is attempted first. When its native surface cannot be created, the partial
/// visual tree retires before windowed hosting is selected. When the surface exists but the
/// composition-hosted browser cannot be bound to it, that browser and the tree retire and a
/// windowed browser is rebuilt from the same authorized environment. Either way a partially
/// bound tree never overlays the windowed browser, and a fixed hosting never changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HostingSelection {
    /// Composition surface effects are still being attempted.
    Probing,
    /// The surface failed; its partial native resources are retiring.
    Retiring,
    /// The surface exists; a composition-hosted browser is being constructed and bound.
    Binding,
    /// Binding failed; the composition browser and surface are retiring before a rebuild.
    Rebuilding,
    /// Browser hosting is fixed for this lifetime.
    Selected(MainHosting),
}

impl HostingSelection {
    /// Begin by attempting composition hosting.
    pub(super) const fn new() -> Self {
        Self::Probing
    }

    /// Construct a composition-hosted browser once its complete native surface exists.
    pub(super) fn surface_ready(&mut self) -> Result<(), DesktopError> {
        self.advance(Self::Probing, Self::Binding, "admit_composition_browser")
    }

    /// Begin retiring a composition surface whose native creation failed.
    pub(super) fn surface_failed(&mut self) -> Result<(), DesktopError> {
        self.advance(Self::Probing, Self::Retiring, "reject_composition")
    }

    /// Fix windowed hosting only after the failed surface has actually retired.
    pub(super) fn partial_retired(&mut self) -> Result<(), DesktopError> {
        self.advance(Self::Retiring, Self::Selected(MainHosting::Windowed), "select_windowed")
    }

    /// Fix composition hosting after the browser is bound and committed to the surface.
    pub(super) fn browser_bound(&mut self) -> Result<(), DesktopError> {
        self.advance(Self::Binding, Self::Selected(MainHosting::Composition), "select_composition")
    }

    /// Begin retiring a composition browser that could not be bound to its surface.
    pub(super) fn bind_failed(&mut self) -> Result<(), DesktopError> {
        self.advance(Self::Binding, Self::Rebuilding, "reject_composition_browser")
    }

    /// Fix windowed hosting once the unbound browser and its surface have actually retired.
    pub(super) fn composition_browser_retired(&mut self) -> Result<(), DesktopError> {
        self.advance(Self::Rebuilding, Self::Selected(MainHosting::Windowed), "rebuild_windowed")
    }

    /// Return the hosting the next browser construction must use, if one may begin.
    pub(super) const fn construction(self) -> Option<MainHosting> {
        match self {
            Self::Binding => Some(MainHosting::Composition),
            Self::Selected(hosting) => Some(hosting),
            Self::Probing | Self::Retiring | Self::Rebuilding => None,
        }
    }

    /// Return the fixed hosting, or nothing while the decision is still pending.
    pub(super) const fn selected(self) -> Option<MainHosting> {
        match self {
            Self::Selected(hosting) => Some(hosting),
            Self::Probing | Self::Retiring | Self::Binding | Self::Rebuilding => None,
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
