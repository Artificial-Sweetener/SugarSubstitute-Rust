// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Initiate ordinary shutdown only after current custody permits atomic admission quiescence.
use super::*;
use sugarsubstitute_contracts::execution::TerminalKind;

/// Passive facts borrowed from the current runner; this grants no admission or retirement proof.
struct Admission {
    requested: bool,
    initial_completed: bool,
    initial_retired: bool,
    intake_ready: bool,
    pending_activation: bool,
    forwarding: bool,
    closing: bool,
    failure: bool,
}
impl Admission {
    /// Call the actual atomic intake owner only after every retained runner obligation permits it.
    fn try_fence(
        self,
        fence: impl FnOnce() -> Result<bool, Box<dyn Error>>,
    ) -> Result<bool, Box<dyn Error>> {
        if !self.requested
            || !self.initial_completed
            || !self.initial_retired
            || !self.intake_ready
            || self.pending_activation
            || self.forwarding
            || self.closing
            || self.failure
        {
            return Ok(false);
        }
        fence()
    }
}
impl<C: EntryActivation> EntryRunner<C> {
    /// Freeze explicit user intent before cleanup can expose a later failure.
    pub(in crate::production) fn accept_user_close(&mut self) {
        if !self.closing && self.failure_source().is_none() {
            self.normal_shutdown = true;
        }
    }
    /// Preserve live runner custody before asking its actual server to fence future entry admission.
    pub(super) fn try_normal_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        let admission = self.normal_admission();
        self.close_after_fence(admission, |runner| {
            runner
                .intake
                .as_mut()
                .ok_or("Actual normal-close intake missing")?
                .try_close_when_idle()
                .map_err(|cause| Box::new(cause) as Box<dyn Error>)
        })
    }

    /// What the runner owes before a normal close: the user's or the child's close, or a restart
    /// into an update prepared before the window opened.
    fn normal_admission(&self) -> Admission {
        Admission {
            requested: self.child.normal_close_requested() || self.restart_owed(),
            initial_completed: self.initial_completed,
            initial_retired: self.transport_completions[1]
                .as_ref()
                .is_some_and(|completion| completion.outcome.kind() == TerminalKind::Success),
            intake_ready: self.intake_ready,
            pending_activation: self.pending_activation.is_some(),
            forwarding: self.forward.is_some(),
            closing: self.closing,
            failure: self.failure.is_some() || self.child.failure_source().is_some(),
        }
    }

    /// Close normally once `admission` permits and `fence` reports the intake idle.
    fn close_after_fence(
        &mut self,
        admission: Admission,
        fence: impl FnOnce(&mut Self) -> Result<bool, Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        let fenced = admission.try_fence(|| fence(self))?;
        if fenced {
            self.accept_user_close();
            self.request_shutdown()?;
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "normal_idle_tests.rs"]
mod tests;
