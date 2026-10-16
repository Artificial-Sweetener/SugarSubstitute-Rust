// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Classify cached normal-idle vetoes without changing the actual close decision.
use super::*;

/// Cached Reporter facts distinguish an exited process from accepted source disposal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production::entry_activation) struct ReporterIdleState {
    pub receiving_delivered: bool,
    pub recycled: bool,
    pub family_retired: bool,
    pub process_present: bool,
    pub binding_present: bool,
    pub relay_idle: bool,
    pub dismiss_requested: bool,
    pub accepted_dismiss: bool,
}
/// The first current veto and its existing identity; no native authority is inferred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) struct IdleGate {
    reason: &'static str,
    identity: IdleIdentity,
    reporter: ReporterIdleState,
}
impl ProductionActivation {
    /// Log only changed cached facts after the actual policy decision is known.
    pub(super) fn observe_idle_gate(
        &self,
        decision: &std::result::Result<ShutdownBudget, IdleBlock>,
    ) {
        let gate = self.qualification_idle_gate(decision);
        if self.idle_gate.replace(Some(gate)) != Some(gate) {
            tracing::info!(operation="root_idle_gate",gate=gate.reason,identity=?gate.identity,
                reporter=?gate.reporter,
                "Normal idle qualification decision changed");
        }
    }
    /// Copy the actual first denial and retained Reporter facts without provider calls.
    fn qualification_idle_gate(
        &self,
        decision: &std::result::Result<ShutdownBudget, IdleBlock>,
    ) -> IdleGate {
        let blocked = decision.as_ref().err().copied().unwrap_or(IdleBlock::global("Eligible"));
        IdleGate {
            reason: blocked.reason,
            identity: blocked.identity,
            reporter: self.incidents.reporting.qualification_idle_state(),
        }
    }
}
#[cfg(test)]
#[path = "qualification_tests.rs"]
mod tests;
