// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Derive the complete selection only from the retained authenticated original frame.
use super::*;
use sugarsubstitute_crash::diagnostic_client::{
    DiagnosticCommand, DiagnosticFrame, DiagnosticIncident,
};
/// This runs on original Core before constructing either native file owner.
#[expect(clippy::result_large_err, reason = "Keep funded export Operation cause inline")]
pub(super) fn validate(input: &ExportInput) -> Result<ReportExportIntent, ExportCause> {
    input.issued.matches_received(&input.received).map_err(ExportCause::Wire)?;
    let protected = input.protected.as_ref().ok_or(ExportCause::State)?;
    let view = input.view.as_ref().ok_or(ExportCause::State)?;
    let frame = input.received.decode().map_err(ExportCause::Wire)?;
    if frame.key != input.received.key() {
        return Err(ExportCause::State);
    }
    selection(frame, protected.operation().owner(), view.view().binding())
}
/// Classify only lexical destination rejection; this grants no authentication or native authority.
pub(super) fn destination_rejection(cause: &ExportCause) -> bool {
    matches!(
        cause,
        ExportCause::Export(ReportExportCause::Rejected(
            sugarsubstitute_crash::report_export::ReportExportCode::Destination
        ))
    )
}
/// Matching facts constrain the selected intent; authentication is checked by `validate`.
#[expect(clippy::result_large_err, reason = "Keep funded export Operation cause inline")]
fn selection(
    frame: DiagnosticFrame<'_>,
    owner: sugarsubstitute_contracts::lifecycle::CoordinatorKey,
    binding: ReportBinding,
) -> Result<ReportExportIntent, ExportCause> {
    if frame.key.role() != DiagnosticRole::Reporter
        || frame.key.owner() != owner
        || frame.context.incident
            != Some(DiagnosticIncident { key: binding.incident(), revision: binding.revision() })
        || frame.context.operation.is_some()
        || frame.context.deadline.is_some()
        || frame.context.at.is_some()
    {
        return Err(ExportCause::State);
    }
    let DiagnosticCommand::SaveSelectionReady { session, selection } = frame.command else {
        return Err(ExportCause::State);
    };
    if session != binding.session() {
        return Err(ExportCause::State);
    }
    ReportExportIntent::new(binding, selection).map_err(ExportCause::Export)
}
#[cfg(test)]
#[path = "authentication/tests.rs"]
mod tests;
