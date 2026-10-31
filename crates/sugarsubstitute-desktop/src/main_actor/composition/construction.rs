// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select browser hosting, then bind the owned composition tree before the frontend navigates.

use super::*;
use crate::main_actor::hosting::HostingSelection;

impl MainActor {
    /// Fix browser hosting before construction begins, one native effect per call.
    ///
    /// The device, HWND target and visual tree are created before any WebView exists. When
    /// one of them fails (for example in a remote session or with an unsupported driver), the
    /// failure is logged with its cause, the partial tree retires, and windowed hosting is
    /// selected; the page then receives no native canvas presentation. Returns true once
    /// hosting is fixed.
    pub(in crate::main_actor) fn select_hosting(&mut self) -> Result<bool, DesktopError> {
        match self.hosting {
            HostingSelection::Selected(_) => Ok(true),
            HostingSelection::Probing => {
                if self.composition.is_none() {
                    let claim =
                        self.request()?.storage_lease().claim(65536).map_err(|cause| {
                            DesktopError::new("claim_browser_composition", cause)
                        })?;
                    self.composition = Some(Composition::new(
                        self.window()?.clone(),
                        claim,
                        self.composition_device,
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

    /// Bind the composition-hosted browser and attach its native input only after commit.
    pub(in crate::main_actor) fn prepare_composition(&mut self) -> Result<bool, DesktopError> {
        if self.composition.as_ref().is_some_and(|composition| composition.controller.is_none()) {
            let controller = self.view()?.controller();
            self.composition
                .as_mut()
                .ok_or_else(|| missing("browser_composition"))?
                .attach_controller(controller)?;
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
