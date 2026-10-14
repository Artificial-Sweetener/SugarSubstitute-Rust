// SPDX-License-Identifier: GPL-3.0-or-later
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
    /// Preserve live runner custody before asking its actual server to fence future entry admission.
    pub(super) fn try_normal_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        let admission = Admission {
            requested: self.child.normal_close_requested(),
            initial_completed: self.initial_completed,
            initial_retired: self.transport_completions[1]
                .as_ref()
                .is_some_and(|completion| completion.outcome.kind() == TerminalKind::Success),
            intake_ready: self.intake_ready,
            pending_activation: self.pending_activation.is_some(),
            forwarding: self.forward.is_some(),
            closing: self.closing,
            failure: self.failure.is_some() || self.child.failure_source().is_some(),
        };
        let fenced = admission.try_fence(|| {
            self.intake
                .as_mut()
                .ok_or("Actual normal-close intake missing")?
                .try_close_when_idle()
                .map_err(|cause| Box::new(cause) as Box<dyn Error>)
        })?;
        if fenced {
            self.request_shutdown()?;
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "normal_idle_tests.rs"]
mod tests;
