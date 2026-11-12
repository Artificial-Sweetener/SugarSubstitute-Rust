// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Export an authenticated native selection while genuine source and receiving jobs remain live.
#[path = "exporting/authentication.rs"]
mod authentication;
#[path = "exporting/operation.rs"]
mod operation;
#[path = "exporting/service.rs"]
mod service;
#[path = "exporting/source.rs"]
mod source;
use super::reporting::PreparedReportView;
pub(in crate::production) use service::{
    ExportCompletionCustody, ExportCompletionParts, ExportIssue, ExportService, ExportTerminal,
};
use sugarsubstitute_contracts::{execution::*, incident::*};
use sugarsubstitute_crash::{
    diagnostic_client::{DiagnosticWireError, IssuedDiagnosticRequest, ReceivedDiagnosticRequest},
    incident::*,
    report_export::{
        ReportExportCause, ReportExportIntent, ReportExportOperation, ReportExportReceipt,
    },
};
use sugarsubstitute_supervision::{ClaimError, committed_reader::*};
/// Fixed controller metadata, deducted from its original2MiB before policy allocation.
pub(in crate::production) const EXPORT_SOURCE_BYTES: u64 = 32 * 1024;
/// Original protected namespace, inseparable prepared view and authenticated original frame.
/// None of the inert binding, request number or destination can grant native authority.
pub(in crate::production) struct ExportInput {
    pub(in crate::production) protected: Option<ProtectedCommittedReadRequest>,
    pub(in crate::production) view: Option<PreparedReportView>,
    pub(in crate::production) received: ReceivedDiagnosticRequest,
    pub(in crate::production) issued: IssuedDiagnosticRequest,
}
/// Preserve the original unexpected cause separately from categorical service decisions.
#[derive(Debug)]
pub(in crate::production) enum ExportCause {
    Admission(AdmissionError),
    Memory(MemoryClaimError),
    Native(ClaimError),
    Wire(DiagnosticWireError),
    Export(ReportExportCause),
    Record(IncidentStoreError),
    Utc(IncidentUtcReadError),
    Reserve(std::collections::TryReserveError),
    State,
}
impl std::fmt::Display for ExportCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Incident export retained its original failure")
    }
}
impl std::error::Error for ExportCause {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(cause) => Some(cause),
            Self::Memory(cause) => Some(cause),
            Self::Native(cause) => Some(cause),
            Self::Wire(cause) => Some(cause),
            Self::Export(cause) => Some(cause),
            Self::Record(cause) => Some(cause),
            Self::Utc(cause) => Some(cause),
            Self::Reserve(cause) => Some(cause),
            Self::State => None,
        }
    }
}
