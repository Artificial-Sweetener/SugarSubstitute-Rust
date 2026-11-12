// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind report selection semantics to the shared receiving-file selection.
use super::ReportExportCause as E;
use crate::diagnostic_client::{DiagnosticRequestId, DiagnosticSelection};
use sugarsubstitute_contracts::incident::{ExportSelection, ReportBinding};
use sugarsubstitute_supervision::ReportFileSelection;
/// Owned immutable bounded selection; construction authenticates no sender.
pub struct ReportExportIntent {
    pub(super) binding: ReportBinding,
    pub(super) request: DiagnosticRequestId,
    pub(super) selection: ExportSelection,
    pub(super) destination: ReportFileSelection,
}
impl ReportExportIntent {
    /// Retain the report binding and exact native-selected path under storage validation.
    /// The adapter matches the selected request against its authenticated session ledger.
    /// Selection data grants no filesystem authority; execution separately admits writes.
    /// # Errors
    /// Returns Destination for malformed or unbounded path data before native effects.
    pub fn new(binding: ReportBinding, selection: DiagnosticSelection<'_>) -> Result<Self, E> {
        let destination = ReportFileSelection::new(
            selection.destination,
            selection.overwrite,
            [binding.session().get(), selection.id.get()],
        )
        .map_err(E::from)?;
        Ok(Self { binding, request: selection.id, selection: selection.selection, destination })
    }
    /// Return full incident/revision/session matching facts.
    pub const fn binding(&self) -> ReportBinding {
        self.binding
    }
    /// Return exact original selected request number, never filesystem authority.
    pub const fn request(&self) -> DiagnosticRequestId {
        self.request
    }
    /// Return actual sensitive dump consent selection.
    pub const fn selection(&self) -> ExportSelection {
        self.selection
    }
}
#[cfg(test)]
mod tests;
