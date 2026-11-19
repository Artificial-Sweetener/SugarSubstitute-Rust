// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Tell this Main's page whether the application has an update report, and hold its request
//! to see it until the application takes it.
use super::*;
use sugarsubstitute_contracts::host_update_report::{
    UpdateReportClientMessage, UpdateReportHostMessage,
};

impl MainActor {
    /// Adopt whether the application has an update report to show; the page is told when that
    /// changes.
    pub(crate) fn observe_update_report(&mut self, available: bool) -> Result<(), DesktopError> {
        if self.update_report.observe(available) { self.tell_update_report_page() } else { Ok(()) }
    }

    /// Whether the page asked for the report since the application last took its request.
    pub(crate) fn take_update_report_request(&mut self) -> bool {
        !self.closing() && self.update_report.take_asked()
    }

    /// Tell the current document whether there is a report. A page that is not revealed, or is
    /// closing, is told when it next asks.
    fn tell_update_report_page(&mut self) -> Result<(), DesktopError> {
        if self.closing()
            || !self.revealed
            || self.readiness.as_ref().and_then(MainReadiness::panel_context).is_none()
        {
            return Ok(());
        }
        let (revision, available) = self.update_report.page_state();
        let command = crate::main_update_report_page::UpdateReportCommand::new(
            self.identity(),
            &UpdateReportHostMessage::State { revision, available },
            self.request()?.storage_lease(),
        )
        .map_err(|cause| DesktopError::boxed("encode_update_report_state", cause))?;
        self.send_update_report_page(command)
    }

    /// Answer the page's watch with the current state, or hold its open for the application
    /// when it names the state still current; any other open is stale and is answered with the
    /// current state.
    pub(super) fn answer_update_report_page(
        &mut self,
        message: UpdateReportClientMessage,
        claim: sugarsubstitute_contracts::execution::MemoryClaim,
    ) -> Result<(), DesktopError> {
        let receipt = crate::PanelIngressReceipt::control(self.identity(), claim)
            .map_err(|cause| DesktopError::new("acknowledge_update_report_message", cause))?;
        self.complete_panel_ingress(receipt)?;
        match message {
            UpdateReportClientMessage::Watch { .. } => self.tell_update_report_page(),
            UpdateReportClientMessage::Open { revision, .. } => {
                if self.update_report.ask_at(revision) {
                    Ok(())
                } else {
                    self.tell_update_report_page()
                }
            }
        }
    }
}
