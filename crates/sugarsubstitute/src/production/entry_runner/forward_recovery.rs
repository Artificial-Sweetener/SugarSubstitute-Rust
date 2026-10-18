// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Return an unsubmitted launch source to election after checked transport retirement.
use super::*;
use sugarsubstitute_contracts::execution::JobOutcome;
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
        self.bootstrap.as_mut().ok_or("Original entry bootstrap missing")?.advance(&self.jobs)?;
        tracing::info!(
            operation = "production_entry_reelection",
            "Unsubmitted launch source returned after native retirement for checked re-election"
        );
        Ok(())
    }
}
