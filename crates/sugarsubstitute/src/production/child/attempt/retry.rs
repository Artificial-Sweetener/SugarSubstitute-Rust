// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Revalidate exact completed attempt custody before restoring the original owned input.
use super::*;
impl ProductionChild {
    /// Inspect retained failure presentation only; this cannot establish settlement.
    pub(in crate::production) fn cleanup_failed(&self) -> bool {
        self.attempt.as_ref().is_some_and(|attempt| attempt.failures.iter().any(Option::is_some))
    }
    /// Reject foreign or unsettled custody before the application replaces its old failure.
    /// This reads the same actual retained transaction; it grants no physical retirement.
    pub(in crate::production) fn ensure_retry_ready(&self, identity: MainIdentity) -> Result<()> {
        if identity.run != self.run
            || identity.launch.get() <= self.identity.launch.get()
            || identity.navigation.get() <= self.identity.navigation.get()
            || self.attempt.as_ref().is_none_or(|a| !a.settled_without_failure())
            || self.main.pending()
            || self.output.pending()
            || self.observation.is_some()
        {
            return Err("Retry requires the exact original attempt settlement".into());
        }
        if self.failure.is_some() {
            return Err("Unresolved original child infrastructure cause prevents retry".into());
        }
        Ok(())
    }
    /// Replace only a settled acquisition attempt without replacing capture or channel.
    pub(in crate::production) fn retry_attempt(
        &mut self,
        identity: MainIdentity,
        prepared: &mut PreparedChildStartup,
    ) -> Result<()> {
        self.ensure_retry_ready(identity)?;
        let mut input = self.main.take_reusable_input()?;
        if let Err(cause) = prepared.restore_main_input(&mut input) {
            self.main.restore_reusable_input(&mut input)?;
            return Err(cause);
        }
        self.failed = false;
        self.identity = identity;
        self.connection_budget = None;
        self.acquire = None;
        self.acquisition_header = None;
        self.attempt = None;
        self.grant_header = None;
        self.shutdown.main_retired = false;
        self.shutdown.main_retirement_ack = false;
        self.shutdown.main_absence_pending = false;
        self.main = main::MainPreparation::new(self.main.settings);
        self.output = output::ChildOutput::new();
        self.snapshot = None;
        Ok(())
    }
}
