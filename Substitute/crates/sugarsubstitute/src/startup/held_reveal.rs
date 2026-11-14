// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Close a window that was never revealed when the coordinator restarts into an update it
//! prepared before the window opened, so the user never sees a window that then closes.
use super::*;
use policy::StartupRoute;
use sugarsubstitute_contracts::startup::StartupEffect;

impl<B: StartupBackend> StartupHost<B> {
    /// Whether `effect` retires the splash or reveals the window while the coordinator restarts
    /// into its update; such a step is held instead of carried out.
    pub(super) fn holds_reveal(&self, effect: &StartupEffect) -> bool {
        let holds =
            matches!(effect, StartupEffect::RetireSplash { .. } | StartupEffect::RevealMain { .. })
                && self.restarting();
        if holds {
            tracing::info!(
                operation = "startup_reveal_held",
                "The window stays hidden while the app restarts into its update"
            );
        }
        holds
    }

    /// Follow the coordinator's restart into its update. A window that was never revealed
    /// closes itself to restart, through the same close as every other restart, so the
    /// coordinator relaunches as soon as it exits; the splash first shows which version it
    /// updates to, and it closes only once the coordinator's own notice covers the screen until
    /// the next copy shows. A window revealed first stays open: its MainRevealed makes the coordinator
    /// withdraw the restart, and a held step is then carried out; the update starts on the next
    /// launch.
    pub(super) fn advance_restart_hold(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Ok(());
        }
        if self.restarting() {
            if self.commands.revealed() {
                return Ok(());
            }
            if !self.update_line_shown() {
                self.invalidate(true);
                return Ok(());
            }
            if !self.restart_covered() {
                return Ok(());
            }
            tracing::info!(
                operation = "startup_restart_before_window",
                "The window was never shown; closing to restart into the prepared update"
            );
            return self.restart_to_apply();
        }
        match self.held_reveal.take() {
            Some(effect) => self.route(StartupRoute::Effect(effect)),
            None => Ok(()),
        }
    }

    /// Whether the splash already shows its update line in place of the loading line, or
    /// shows nothing that could: it is not drawn yet, not visible, or the version is one it
    /// cannot show.
    fn update_line_shown(&self) -> bool {
        use sugarsubstitute_splash::StartupStatus::{Loading, LoadingExtended, LoadingLong};
        if !self.prepared
            || !self.visible
            || window_content::updating(
                &self.child,
                Loading,
                self.view.as_ref().is_some_and(StartupView::retried),
            )
            .is_none()
        {
            return true;
        }
        self.preparation
            .as_ref()
            .and_then(crate::preparation::PreparationDriver::published_appearance)
            .is_some_and(|shown| !matches!(shown.status, Loading | LoadingLong | LoadingExtended))
    }

    /// Whether the coordinator's own restart notice is on screen, so closing this splash never
    /// leaves the screen empty before the next copy shows.
    fn restart_covered(&self) -> bool {
        self.child
            .update_status()
            .is_some_and(sugarsubstitute_contracts::update::status::UpdateStatus::restart_covered)
    }

    /// Whether a "Restart to apply" close may hide the window: the coordinator's restart notice
    /// covers the screen, or no coordinator status reaches this window to wait for.
    pub(super) fn restart_close_covered(&self) -> bool {
        self.child
            .update_status()
            .is_none_or(sugarsubstitute_contracts::update::status::UpdateStatus::restart_covered)
    }

    fn restarting(&self) -> bool {
        self.child
            .update_status()
            .is_some_and(sugarsubstitute_contracts::update::status::UpdateStatus::restarting)
    }
}
