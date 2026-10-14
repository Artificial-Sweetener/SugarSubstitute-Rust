// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual report duties before the coordinator's existing normal-idle decision.
use super::*;
impl IncidentReporting {
    /// Pending is a request veto, never a native, filesystem or worker retirement proof.
    /// Delivered original completions may remain retained with their causes and claims.
    pub(in crate::production::entry_activation) fn diagnostic_custody_pending(&self) -> bool {
        !self.receiving_session.delivered()
            || self.launch_input.is_some()
            || self.session.is_some()
            || self.relay.as_ref().is_some_and(|relay| {
                !(self.family_retired || self.binding.is_none() && relay.idle_funded())
            })
            || self.process.is_some()
            || self.store.is_some()
            || self.request.is_some()
            || self.completion.is_some()
            || self.selected.is_some()
            || self.export_work.is_some()
            || self.pending_control.is_some()
            || self.retained_effect.is_some()
            || self.relay_needed
            || self.rejected_completion.is_some()
            || self.rejected_received.is_some()
            || self.symbols.as_ref().is_some_and(|symbols| !symbols.settled())
    }
}
