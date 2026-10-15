// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Dispose checked old session custody only after its genuine receiving adoption.
use super::*;
impl IncidentReporting {
    pub(super) fn advance_recycled_retirement(
        &mut self,
        host: &mut IncidentHost,
        resources: &mut EntryResources<'_>,
    ) -> Result<()> {
        let Some(original) = self.receiving_session.recycled_mut() else {
            return Err("Actual recycled Reporter custody missing".into());
        };
        let relay = self.relay.as_mut().ok_or("Actual recycled Reporter source missing")?;
        let registry =
            resources.authority.registry_mut().ok_or("Actual Reporter registry missing")?;
        if original.receipt.is_none() {
            let key = self.key.ok_or("Actual old Reporter key missing")?;
            match relay.detach_closed(registry, key) {
                Ok(receipt) => original.receipt = receipt,
                Err(ReporterSourceIssue::Busy) => return Ok(()),
                Err(cause) => return Err(cause.into()),
            }
        }
        let Some(receipt) = &original.receipt else { return Ok(()) };
        if !matches!(
            receipt.native_outcome(),
            Some(TerminalKind::Cancelled | TerminalKind::Success)
        ) || self.key != Some(receipt.key())
            || self.binding != Some(original.binding)
        {
            return Err("Actual detached Reporter correlation changed".into());
        }
        if !original.registered {
            let process = receipt.process().ok_or("Actual detached Reporter process missing")?;
            if self.process != Some(process) {
                return Err("Actual detached Reporter family changed".into());
            }
            while registry.take_observation(process)?.is_some() {}
            registry.retire_settled(process)?;
            original.registered = true;
            self.process = None;
        }
        if host.closing() {
            self.family_retired = true;
            self.store = None;
            return Ok(());
        }
        if !self.retain_current_restart(host)? {
            return Ok(());
        }
        let retired =
            self.receiving_session.retired().ok_or("Actual registered Reporter receipt missing")?;
        match host.observe_reporter_retired(&retired) {
            Ok(()) => {}
            Err(crate::production::incident_host::HostIssue::Pending) => return Ok(()),
            Err(cause) => return Err(cause.into()),
        }
        self.family_retired = true;
        self.store = None;
        self.finish_accepted_session()
    }
    /// A requested durable restart retains the original receipt for genuine installed ingress.
    pub(super) fn finish_accepted_session(&mut self) -> Result<()> {
        if !self.family_retired
            || self.failure().is_some()
            || self.dismiss_action == Some(ReportAction::DismissAndRequestRestart)
            || matches!(
                self.retained_effect,
                Some(
                    IncidentEffect::ForwardRestartIntent { .. }
                        | IncidentEffect::ForwardRecoveredRestartIntent { .. }
                )
            )
        {
            return Ok(());
        }
        let dismiss = if self.dismiss_action == Some(ReportAction::Dismiss) {
            Some(self.stopping.ok_or("Original Dismiss shutdown budget missing")?)
        } else {
            None
        };
        let relay = self.relay.as_mut().ok_or("Actual recycled Reporter relay missing")?;
        if !self.receiving_session.finish_disposal(relay, dismiss)? {
            return Ok(());
        }
        self.reset_session_fields();
        Ok(())
    }
    /// Preserve the original accepted interval; no clock or replacement budget is consulted.
    pub(in crate::production::entry_activation) fn normal_close_budget(
        &self,
    ) -> Option<ShutdownBudget> {
        self.receiving_session.normal_close_budget()
    }
    /// Clear only disposed session state; the actual reclaimed source and queued effect stay owned.
    pub(super) fn reset_session_fields(&mut self) {
        self.key = None;
        self.binding = None;
        self.locale = None;
        self.original = None;
        self.report_budget = None;
        self.queue_until = None;
        self.reporter_startup = None;
        self.launch_input = None;
        self.session = None;
        self.session_installed = false;
        self.launched = false;
        self.process = None;
        self.wire_request = 0;
        self.store = None;
        self.request = None;
        self.completion = None;
        self.selected = None;
        self.export_work = None;
        self.pending_control = None;
        self.native_retired = false;
        self.initial_view_admitted = false;
        self.authenticated_peer_observed = None;
        self.stopping = None;
        self.family_retired = false;
        self.dismiss_action = None;
        self.close_counter = None;
        self.relay_until = None;
        self.relay_needed = false;
        self.rejected_completion = None;
        self.rejected_received = None;
        self.close_request = None;
        self.symbols = None;
    }
}
