// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow genuine Bootstrap owners for optional preview without changing startup policy.
use super::ChildBootstrap;
use sugarsubstitute_application::startup::StartupSession;
use sugarsubstitute_contracts::{execution::Moment, startup::MainIdentity};
use sugarsubstitute_desktop::{DesktopStartupCommand, DesktopStartupObservation};

impl ChildBootstrap {
    /// The actual private readiness/registration state precedes source-loan production.
    pub(in crate::production::child) fn prepare_registered_preview(&mut self) {
        if self.closing || !self.capture.ready() || !self.channel.registered() {
            return;
        }
        let (Some(client), Some(native), Some(session), Some(run)) = (
            self.client.as_ref(),
            self.native_scope.as_ref(),
            self.capture.registered_session(),
            self.qualification_run,
        ) else {
            return;
        };
        self.qualification.prepare_preview(client, native, session, run);
    }

    /// Policy has already accepted this exact current MainRevealed as Running.
    pub(in crate::production::child) fn preview_after_main_revealed(
        &mut self,
        identity: MainIdentity,
        session: &StartupSession,
    ) -> Option<DesktopStartupCommand> {
        if self.closing {
            return None;
        }
        let (Some(client), Some(ordinary), Some(native)) =
            (self.client.as_ref(), self.scope.as_ref(), self.native_scope.as_ref())
        else {
            return None;
        };
        let now = match client.now() {
            Ok(now) => now,
            Err(cause) => {
                self.qualification.reject_preview_clock(cause);
                return None;
            }
        };
        self.qualification.preview_after_main_revealed(identity, session, ordinary, native, now)
    }

    /// Only optional capture kinds are consumed here; normal observations remain unchanged.
    pub(in crate::production::child) fn accept_preview_observation(
        &mut self,
        observation: &mut Option<DesktopStartupObservation>,
    ) -> bool {
        let (Some(client), Some(ordinary), Some(jobs)) =
            (self.client.as_ref(), self.scope.as_ref(), self.jobs.as_ref())
        else {
            return self.qualification.retain_unarmed_preview(observation);
        };
        self.qualification.accept_preview(observation, client, ordinary, jobs)
    }

    /// Poll through the existing execution clock; a clock failure retains its first cause.
    pub(in crate::production::child) fn poll_registered_preview(&mut self) {
        if !self.qualification.preview_needs_poll() {
            return;
        }
        let Some(client) = self.client.as_ref() else {
            return;
        };
        match client.now() {
            Ok(now) => self.qualification.poll_preview(now),
            Err(cause) => self.qualification.reject_preview_clock(cause),
        }
    }

    /// Borrow only the once-frozen pending campaign cutoff; this performs no conversion.
    pub(in crate::production::child) fn preview_deadline(&self) -> Option<Moment> {
        self.qualification.preview_deadline()
    }

    /// The original bounded queue has accepted this exact Main retirement command.
    pub(in crate::production::child) fn retire_preview_main(&mut self, identity: MainIdentity) {
        self.qualification.retire_preview_main(identity);
    }

    /// A bounded native command queue rejection grants no new capture or clock.
    pub(in crate::production::child) fn reject_preview_command(&mut self) {
        self.qualification.reject_preview_command();
    }
}
