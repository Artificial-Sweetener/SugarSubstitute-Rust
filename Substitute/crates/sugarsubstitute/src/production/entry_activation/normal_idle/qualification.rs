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
    role: Option<RoleCloseState>,
}
/// Independent cached close owners; pipe retirement does not imply profile retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RoleCloseState {
    close_requested: bool,
    parent_settled: bool,
    normal_ready: bool,
    profile: Option<sugarsubstitute_supervision::profiles::ProfileSnapshot>,
    custody_release_pending: bool,
    output_settled: bool,
    retired: bool,
    closed: bool,
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
                reporter=?gate.reporter,role=?gate.role,
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
            role: self.records.iter().flatten().find_map(|held| {
                let record = &held.record;
                if blocked.identity != IdleIdentity::Run(record.run)
                    && (blocked.reason != "Closing" || record.closed)
                {
                    return None;
                }
                Some(RoleCloseState {
                    close_requested: record
                        .parent
                        .as_ref()
                        .is_some_and(|parent| parent.close_progress_request().is_some()),
                    parent_settled: record.parent.as_ref().is_some_and(|parent| parent.settled()),
                    normal_ready: record
                        .parent
                        .as_ref()
                        .is_some_and(|parent| parent.normal_cleanup_ready()),
                    profile: record.start.as_ref().map(|start| start.profile.snapshot()),
                    custody_release_pending: record
                        .start
                        .as_ref()
                        .is_some_and(|start| start.profile.custody_release_pending()),
                    output_settled: record.start.as_ref().is_none_or(|start| {
                        start.output.as_ref().is_none_or(|output| output.settled())
                    }),
                    retired: record.retired.is_some(),
                    closed: record.closed,
                })
            }),
        }
    }
}
#[cfg(test)]
#[path = "qualification_tests.rs"]
mod tests;
