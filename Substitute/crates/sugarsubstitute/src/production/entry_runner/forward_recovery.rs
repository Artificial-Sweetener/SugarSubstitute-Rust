// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Return a forwarded launch source to election after checked transport retirement, and
//! decide when forwarding to a running copy needs something on screen.
use super::*;
use sugarsubstitute_contracts::execution::JobOutcome;
use sugarsubstitute_supervision::coordinator_entry::EntryPhase;
#[path = "forward_recovery_wait.rs"]
pub(crate) mod wait;
impl<C: EntryActivation> EntryRunner<C> {
    pub(super) fn recover_unsubmitted_forward(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing
            || !self.transport_completions[2]
                .as_ref()
                .is_some_and(|completion| matches!(completion.outcome, JobOutcome::Success(_)))
        {
            return Ok(());
        }
        let Some(forward) = self.forward.as_ref() else { return Ok(()) };
        let mut returned = forward.control.take_unsubmitted_request();
        if returned.is_none() {
            return Ok(());
        }
        let entry = self
            .bootstrap
            .as_mut()
            .and_then(EntryBootstrap::owner_mut)
            .ok_or("Original coordinator entry owner missing")?;
        entry.recover_unsubmitted_client(&mut returned)?;
        self.forward = None;
        self.transport_completions[2] = None;
        if let Some(wait) = &mut self.forward_wait {
            wait.reelected();
        }
        self.bootstrap.as_mut().ok_or("Original entry bootstrap missing")?.advance(&self.jobs)?;
        tracing::info!(
            operation = "production_entry_reelection",
            "Unsubmitted launch source returned after native retirement for checked re-election"
        );
        Ok(())
    }

    /// Advance the wait on another copy: dismiss its surface once the copy took this launch
    /// or this launch won election, act on the user's choice, and probe a copy that has not
    /// answered. Cancel ends this launch without starting it.
    /// # Errors
    /// Preserves job submission and clock failures.
    pub(super) fn advance_forward_wait(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Ok(());
        }
        let won = self.forward.is_none()
            && self.bootstrap.as_ref().and_then(EntryBootstrap::owner).is_some_and(|owner| {
                matches!(owner.snapshot().phase, EntryPhase::ElectionHeld | EntryPhase::Serving)
            });
        let (Some(wait), Some(client), Some(finite)) =
            (&mut self.forward_wait, &self.client, &self.finite)
        else {
            return Ok(());
        };
        if self.initial_completed || won {
            wait.resolve();
            return Ok(());
        }
        let now = client.now()?;
        let step = wait.advance(
            now,
            (client, finite, &self.jobs),
            wait::NATIVE_PROCESS,
            &wait::UnnamedClaimHolder,
        )?;
        wait.schedule_native(now, (self.native_now)());
        if step == wait::WaitStep::Cancel {
            tracing::info!(
                operation = "entry_forward_cancelled",
                "The user cancelled this launch while it waited for the other copy"
            );
            self.initial_completed = true;
            self.request_shutdown()?;
        }
        Ok(())
    }

    /// The waiting surface, once this launch has waited two seconds for the other copy.
    pub(super) fn take_forward_presentation(
        &mut self,
    ) -> Option<sugarsubstitute_desktop::HeadlessPresentation> {
        if self.closing {
            return None;
        }
        let now = self.client.as_ref()?.now().ok()?;
        let controller = self.forward_wait.as_mut()?.take_surface(now)?;
        tracing::info!(
            operation = "entry_forward_presented",
            "This launch is still waiting for the other copy; showing why"
        );
        let locale = self.child.presentation_locale().map_or("en", |locale| locale.as_str());
        Some(sugarsubstitute_desktop::HeadlessPresentation {
            copy: sugarsubstitute_contracts::localization::messages(locale),
            title: sugarsubstitute::APPLICATION_NAME.to_owned(),
            controller: Box::new(controller),
        })
    }
}
