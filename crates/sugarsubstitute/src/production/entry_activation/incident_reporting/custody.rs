// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual report duties before the coordinator's existing normal-idle decision.
use super::*;
impl IncidentReporting {
    /// Copy retained Reporter state without native queries or receiving transitions.
    #[cfg(any(test, feature = "k1-native-qualification"))]
    pub(in crate::production::entry_activation) fn qualification_idle_state(
        &self,
    ) -> super::super::normal_idle::qualification::ReporterIdleState {
        super::super::normal_idle::qualification::ReporterIdleState {
            receiving_delivered: self.receiving_session.delivered(),
            recycled: self.receiving_session.recycled().is_some(),
            family_retired: self.family_retired,
            process_present: self.process.is_some(),
            binding_present: self.binding.is_some(),
            relay_idle: self.relay.as_ref().is_some_and(ReporterRelay::idle_funded),
            dismiss_requested: self.dismiss_action.is_some(),
            accepted_dismiss: self.receiving_session.normal_close_budget().is_some(),
        }
    }
    /// Classify cached receiving custody without polling or disposing the Reporter.
    pub(in crate::production::entry_activation) fn idle_block(&self) -> Option<&'static str> {
        if !self.receiving_session.delivered() {
            return Some("ReporterReceivingDelivery");
        }
        if self.launch_input.is_some() {
            return Some("ReporterLaunchInput");
        }
        if self.session.is_some() {
            return Some("ReporterSession");
        }
        if self.relay.as_ref().is_some_and(|relay| {
            !(self.family_retired || self.binding.is_none() && relay.idle_funded())
        }) {
            return Some("ReporterRelay");
        }
        if self.process.is_some() {
            return Some("ReporterProcess");
        }
        if self.store.is_some() {
            return Some("ReporterStore");
        }
        if self.request.is_some() {
            return Some("ReporterRequest");
        }
        if self.completion.is_some() {
            return Some("ReporterCompletion");
        }
        if self.selected.is_some() {
            return Some("ReporterSelection");
        }
        if self.export_work.is_some() {
            return Some("ReporterExport");
        }
        if self.pending_control.is_some() {
            return Some("ReporterControl");
        }
        if self.retained_effect.is_some() {
            return Some("ReporterEffect");
        }
        if self.relay_needed {
            return Some("ReporterRelayNeeded");
        }
        if self.rejected_completion.is_some() {
            return Some("ReporterRejectedCompletion");
        }
        if self.rejected_received.is_some() {
            return Some("ReporterRejectedReceived");
        }
        if self.symbols.as_ref().is_some_and(|symbols| !symbols.settled()) {
            return Some("ReporterSymbols");
        }
        None
    }
}

#[cfg(test)]
mod qualification_tests {
    use super::*;
    #[test]
    fn normal_idle_gate_reporter_cached_veto_matches_pending_and_dismiss_is_not_disposal() {
        let mut reporting = IncidentReporting::new();
        assert_eq!(reporting.idle_block(), None);
        reporting.relay_needed = true;
        assert_eq!(reporting.idle_block(), Some("ReporterRelayNeeded"));
        assert!(reporting.idle_block().is_some());
        reporting.dismiss_action = Some(ReportAction::Dismiss);
        let state = reporting.qualification_idle_state();
        assert!(state.dismiss_requested);
        assert!(!state.accepted_dismiss);
        reporting.relay_needed = false;
        assert_eq!(reporting.idle_block(), None);
        assert!(reporting.normal_close_budget().is_none());
    }
}
