// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select browser hosting, then bind the owned composition tree before the controller is observed.

use super::*;
use crate::main_actor::hosting::HostingSelection;

/// One bounded step of binding the composition-hosted browser to its surface.
pub(in crate::main_actor) enum BindStep {
    /// A native effect ran; call again on a later native drain.
    Pending,
    /// The browser is bound and committed; its controller may now be observed.
    Bound,
    /// Binding failed; the unbound browser and its surface must retire before a windowed rebuild.
    Rebuild,
}

impl MainActor {
    /// Fix the first browser construction's hosting before any WebView exists.
    ///
    /// The device, HWND target and visual tree are created one native effect per call. When
    /// one of them fails (for example in a remote session or with an unsupported driver), the
    /// failure is logged with its cause, the partial tree retires, and windowed hosting is
    /// selected. Returns true once a browser construction may begin.
    pub(in crate::main_actor) fn select_hosting(&mut self) -> Result<bool, DesktopError> {
        match self.hosting {
            HostingSelection::Selected(_) | HostingSelection::Binding => Ok(true),
            HostingSelection::Rebuilding => {
                Err(DesktopError::message("select_hosting", "composition browser is retiring"))
            }
            HostingSelection::Probing => {
                if self.composition.is_none() {
                    let claim =
                        self.request()?.storage_lease().claim(65536).map_err(|cause| {
                            DesktopError::new("claim_browser_composition", cause)
                        })?;
                    self.composition = Some(Composition::new(
                        self.window()?.clone(),
                        claim,
                        self.composition_source,
                    ));
                }
                let composition =
                    self.composition.as_mut().ok_or_else(|| missing("browser_composition"))?;
                match composition.advance() {
                    Ok(_) if composition.surface_ready() => {
                        self.hosting.surface_ready()?;
                        Ok(true)
                    }
                    Ok(_) => Ok(false),
                    Err(cause) => {
                        tracing::warn!(operation="main_composition_unavailable",
                            identity=?self.identity(), cause=%cause,
                            "Browser composition is unavailable; hosting the main browser windowed");
                        self.hosting.surface_failed()?;
                        Ok(false)
                    }
                }
            }
            HostingSelection::Retiring => {
                let retired =
                    self.composition.as_mut().map_or(Ok(true), Composition::retire_step)?;
                if retired {
                    self.composition = None;
                    self.hosting.partial_retired()?;
                }
                Ok(retired)
            }
        }
    }

    /// Bind the constructed composition-hosted browser before the parent observes its controller.
    ///
    /// A binding failure is logged with its cause and retains the already authorized
    /// environment, so the windowed rebuild needs no second environment authorization.
    pub(in crate::main_actor) fn bind_browser(&mut self) -> Result<BindStep, DesktopError> {
        if self.composition.as_ref().is_some_and(|composition| composition.controller.is_none()) {
            let controller = self.view()?.controller();
            self.composition
                .as_mut()
                .ok_or_else(|| missing("browser_composition"))?
                .attach_controller(controller)?;
        }
        let bound =
            self.composition.as_mut().ok_or_else(|| missing("browser_composition"))?.advance();
        match bound {
            Ok(true) => {
                self.hosting.browser_bound()?;
                Ok(BindStep::Bound)
            }
            Ok(false) => Ok(BindStep::Pending),
            Err(cause) => {
                tracing::warn!(operation="main_composition_bind_unavailable",
                    identity=?self.identity(), cause=%cause,
                    "Browser composition binding failed; rebuilding the main browser windowed");
                self.reused_environment = Some(self.view()?.environment());
                self.hosting.bind_failed()?;
                Ok(BindStep::Rebuild)
            }
        }
    }

    /// Retire the unbound surface, then its browser, before the windowed rebuild begins.
    /// Returns true once both have actually retired and windowed hosting is fixed.
    pub(in crate::main_actor) fn retire_composition_browser(
        &mut self,
    ) -> Result<bool, DesktopError> {
        if let Some(composition) = &mut self.composition {
            if composition.retire_step()? {
                self.composition = None;
            }
            return Ok(false);
        }
        if let Some(view) = &mut self.view {
            if view
                .checked_retire()
                .map_err(|cause| DesktopError::new("retire_composition_browser", cause))?
            {
                self.view = None;
            }
            return Ok(false);
        }
        self.hosting.composition_browser_retired()?;
        Ok(true)
    }

    /// Forward native input to the bound browser only after the parent authorized it.
    pub(in crate::main_actor) fn attach_composition_input(&mut self) -> Result<(), DesktopError> {
        let events = self.events()?.clone();
        self.composition
            .as_mut()
            .ok_or_else(|| missing("browser_composition"))?
            .prepare_input(events)
    }
}
