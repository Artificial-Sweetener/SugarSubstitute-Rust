// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain each actual surface slot before the next fallible initialization boundary.

use crate::DesktopError;

/// Supply native creation effects while the caller retains every completed return value.
pub(super) trait SurfacePort {
    /// Exact retained native window type.
    type Window;
    /// Exact retained CPU presentation owner.
    type Presenter;
    /// Inert native control owner capable of retaining partial initialization.
    type Controls;
    /// Return the actual hidden native window for immediate outer adoption.
    fn window(&mut self) -> Result<Self::Window, DesktopError>;
    /// Record the window boundary only after its owner was adopted.
    fn window_adopted(&mut self) -> Result<(), DesktopError>;
    /// Configure presentation while the exact parent window stays retained.
    fn presenter(&mut self, window: &Self::Window) -> Result<Self::Presenter, DesktopError>;
    /// Record successful presentation-owner construction after adoption.
    fn presenter_adopted(&mut self) -> Result<(), DesktopError>;
    /// Construct inert controls before any subclass or child HWND creation.
    fn controls(&mut self, window: &Self::Window) -> Result<Self::Controls, DesktopError>;
    /// Initialize the control owner already retained by the surface.
    fn initialize_controls(&mut self, controls: &mut Self::Controls) -> Result<(), DesktopError>;
    /// Record completed native control construction after its actual return.
    fn controls_adopted(&mut self) -> Result<(), DesktopError>;
}

/// One attempt fence shared by the actual surface and deterministic native-boundary tests.
pub(super) struct Initialization {
    attempted: bool,
}
impl Initialization {
    /// Construct inertly; surface slots are already held by the outer native session.
    pub(super) const fn new() -> Self {
        Self { attempted: false }
    }

    /// Adopt returns directly into retained slots, including the inert controls before effects.
    pub(super) fn initialize<P: SurfacePort>(
        &mut self,
        window: &mut Option<P::Window>,
        presenter: &mut Option<P::Presenter>,
        controls: &mut Option<P::Controls>,
        port: &mut P,
    ) -> Result<(), DesktopError> {
        if self.attempted || window.is_some() || presenter.is_some() || controls.is_some() {
            return Err(DesktopError::message(
                "initialize_startup_surface",
                "surface initialization already attempted",
            ));
        }
        self.attempted = true;
        *window = Some(port.window()?);
        port.window_adopted()?;
        let retained = window.as_ref().ok_or_else(|| {
            DesktopError::message("initialize_startup_surface", "retained window missing")
        })?;
        *presenter = Some(port.presenter(retained)?);
        port.presenter_adopted()?;
        *controls = Some(port.controls(retained)?);
        port.initialize_controls(controls.as_mut().ok_or_else(|| {
            DesktopError::message("initialize_startup_surface", "retained controls missing")
        })?)?;
        port.controls_adopted()
    }
}

#[cfg(test)]
#[path = "startup_surface_custody_tests.rs"]
mod tests;
