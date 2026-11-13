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

    /// Acknowledge the newest update status to the coordinator, then carry out a held step once
    /// the coordinator withdrew its restart; the update then starts on the next launch.
    pub(super) fn advance_restart_hold(&mut self) -> Result<(), Box<dyn Error>> {
        self.acknowledge_update_status()?;
        if self.closing || self.restarting() {
            return Ok(());
        }
        match self.held_reveal.take() {
            Some(effect) => self.route(StartupRoute::Effect(effect)),
            None => Ok(()),
        }
    }

    /// Tell the coordinator the window applied its newest status. A status saying the app
    /// restarts is acknowledged only while the window has never been revealed: from then on
    /// every reveal is held, so the coordinator may close it. A window revealed first never
    /// acknowledges it, and its MainRevealed makes the coordinator withdraw the restart.
    fn acknowledge_update_status(&mut self) -> Result<(), Box<dyn Error>> {
        let revision = self.child.update_status_revision();
        if revision <= self.status_acknowledged || (self.restarting() && self.commands.revealed()) {
            return Ok(());
        }
        self.child.acknowledge_update_status(revision)?;
        self.status_acknowledged = revision;
        Ok(())
    }

    fn restarting(&self) -> bool {
        self.child
            .update_status()
            .is_some_and(sugarsubstitute_contracts::update::status::UpdateStatus::restarting)
    }
}
