// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep reveal, native geometry events and caption actions on the retained main peer thread.

use super::*;
use tao::event::WindowEvent;

impl MainActor {
    /// Reveal only after the application has accepted readiness and actual splash retirement.
    pub(crate) fn reveal(
        &mut self,
        identity: crate::MainIdentity,
    ) -> Result<DesktopStartupObservation, DesktopError> {
        self.admission.frontend(identity)?;
        if self.revealed || !self.readiness.as_ref().is_some_and(MainReadiness::ready) {
            return Err(DesktopError::message(
                "reveal_main",
                "main readiness is missing or already revealed",
            ));
        }
        self.view()?
            .set_visible(true)
            .map_err(|cause| DesktopError::new("show_main_view", cause))?;
        self.show_placed_window()?;
        self.window()?.set_focus();
        self.view()?.focus().map_err(|cause| DesktopError::new("focus_main_view", cause))?;
        // Native show/focus can resize the container before Tao delivers its queued resize event.
        // Restore full-window browser bounds and raise retained caption peers before publication.
        self.resize()?;
        self.confirm_visible()?;
        self.receivers()?.reveal_navigation();
        self.revealed = true;
        self.placement.changed();
        self.panel_transport
            .synchronize(self.readiness.as_ref().and_then(MainReadiness::panel_context));
        #[cfg(feature = "k1-native-qualification")]
        tracing::info!(operation="main_revealed", identity=?identity,
            "Actual ready Main reveal completed after splash retirement");
        #[cfg(feature = "k1-native-qualification")]
        crate::qualification_trace::progress(
            crate::qualification_trace::Progress::MainRevealed,
            identity,
        );
        Ok(DesktopStartupObservation::MainRevealed(identity))
    }

    /// Route only this actor's main window; caption/native controls share one owner.
    pub(crate) fn window_event(
        &mut self,
        id: tao::window::WindowId,
        event: &WindowEvent<'_>,
    ) -> Result<Option<DesktopStartupObservation>, DesktopError> {
        if self.window.as_ref().is_none_or(|window| window.id() != id) {
            return Ok(None);
        }
        if self.admission.closing() {
            return Ok(None);
        }
        if matches!(
            event,
            WindowEvent::Moved(_)
                | WindowEvent::Resized(_)
                | WindowEvent::ScaleFactorChanged { .. }
        ) {
            self.placement.changed();
        }
        match event {
            WindowEvent::Destroyed => {
                self.placement.close();
                return close_request::observe(self.revealed);
            }
            WindowEvent::CloseRequested => {
                return close_request::observe(self.revealed);
            }
            // Activation focuses this top-level window; a windowed browser takes it from here.
            WindowEvent::Focused(true) => {
                if let Some(focus) = &self.windowed_focus {
                    focus.enter();
                }
            }
            WindowEvent::Resized(_) => self.resize()?,
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => self.rescale(*scale_factor)?,
            #[cfg(feature = "native-composition")]
            WindowEvent::Moved(_) => self.composition_moved()?,
            _ => {}
        }
        Ok(None)
    }
    /// Route caption actions through the same visibility and failure policy as native events.
    pub(crate) fn activate(
        &self,
        id: u16,
    ) -> Result<Option<DesktopStartupObservation>, DesktopError> {
        if self.admission.closing() {
            return Ok(None);
        }
        match id {
            1 => self.window()?.set_minimized(true),
            2 => self.window()?.set_maximized(!self.window()?.is_maximized()),
            3 => return close_request::observe(self.revealed),
            _ => {
                return Err(DesktopError::message("main_control", "unknown caption control"));
            }
        }
        Ok(None)
    }
}
