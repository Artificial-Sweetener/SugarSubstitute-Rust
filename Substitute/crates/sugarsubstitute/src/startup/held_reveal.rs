// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the splash and the hidden window while the coordinator restarts into an update it
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

    /// Carry out a held step once the coordinator withdrew its restart; the update then starts
    /// on the next launch.
    pub(super) fn release_held_reveal(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing || self.restarting() {
            return Ok(());
        }
        match self.held_reveal.take() {
            Some(effect) => self.route(StartupRoute::Effect(effect)),
            None => Ok(()),
        }
    }

    fn restarting(&self) -> bool {
        self.child
            .update_status()
            .is_some_and(sugarsubstitute_contracts::update::status::UpdateStatus::restarting)
    }
}
