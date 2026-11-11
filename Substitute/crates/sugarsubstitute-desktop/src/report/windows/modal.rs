// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Execute one genuinely admitted Save only outside Tao's event-handler borrow.
use super::super::{ReportSurfaceObservation, state::Request};
use super::*;
use sugarsubstitute_contracts::incident::ExportSelection;
/// One exact state-admitted request; this inert slot creates no filesystem authority.
pub(super) struct PreparedSave {
    pub(super) request: Request,
    pub(super) selection: ExportSelection,
}
impl session::Session {
    pub(super) fn prepare_save(
        &mut self,
        request: Request,
        selection: ExportSelection,
    ) -> Result<(), ReportSurfaceError> {
        self.state.matching_request(request.id, request.binding)?;
        if self.prepared_save.is_some() || self.state.closing() {
            return Err(ReportSurfaceError::rejected("report_picker_dispatch_busy"));
        }
        self.prepared_save = Some(PreparedSave { request, selection });
        Ok(())
    }
    /// The supported native message hook owns the sole Session/content loan through this call.
    pub(super) fn run_prepared_save(
        &mut self,
        content: &mut impl ReportSessionContent,
        cancel: Rc<dispatch::ModalCancellation>,
    ) -> Result<(), ReportSurfaceError> {
        let Some(prepared) = self.prepared_save.take() else { return Ok(()) };
        if self.state.closing() {
            return Ok(());
        }
        self.state.matching_request(prepared.request.id, prepared.request.binding)?;
        let controls = self
            .controls
            .as_ref()
            .ok_or_else(|| ReportSurfaceError::rejected("report_picker_native_owner"))?;
        let outcome = save_dialog::select(
            self.hwnd,
            prepared.request,
            prepared.selection,
            self.labels,
            &controls.lease,
            content,
            cancel,
        )?;
        self.picker = outcome.retirement;
        if let Some(cause) = outcome.failure {
            return Err(cause);
        }
        if let Some(selection) = outcome.selection {
            content.observe(ReportSurfaceObservation::SaveSelected(selection))?;
        } else {
            self.state.completed_request();
            content.observe(ReportSurfaceObservation::SaveCancelled {
                request: prepared.request.id,
                binding: prepared.request.binding,
            })?;
        }
        if let Some(update) = outcome.update {
            let observed = outcome
                .update_at
                .ok_or_else(|| ReportSurfaceError::rejected("report_picker_update_time"))?;
            self.apply_at(content, update, observed)?;
        } else if let Some(controls) = &self.controls {
            controls.enable(self.state.actionable(), self.state.restart())?;
        }
        Ok(())
    }
}
