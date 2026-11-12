// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Return a forwarded launch source to election after checked transport retirement, and
//! decide when forwarding to a running copy needs something on screen.
use super::*;
use sugarsubstitute_contracts::execution::JobOutcome;
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

    /// Present the running copy's state once forwarding has a reason to show something.
    /// The cause ends this launch's pump; the application then shows the question or the
    /// waiting surface and starts a fresh launch when the running copy has gone.
    /// # Errors
    /// Returns the retained presentation cause, or a probe job's failure.
    pub(super) fn advance_forward_wait(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing || self.initial_completed {
            return Ok(());
        }
        let (Some(wait), Some(client), Some(finite)) =
            (&mut self.forward_wait, &self.client, &self.finite)
        else {
            return Ok(());
        };
        let now = client.now()?;
        let busy = wait.advance(
            now,
            (client, finite, &self.jobs),
            sugarsubstitute_desktop::probe_process_responsiveness,
        )?;
        wait.schedule_native(now, (self.native_now)());
        if let Some(busy) = busy {
            tracing::info!(
                operation = "entry_forward_presented",
                state = ?busy.state,
                "The running copy did not take this launch; showing why"
            );
            self.retain(Box::new(busy));
            return Err("The running copy did not take this launch".into());
        }
        Ok(())
    }
}
