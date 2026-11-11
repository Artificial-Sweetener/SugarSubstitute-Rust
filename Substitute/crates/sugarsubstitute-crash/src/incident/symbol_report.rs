// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve one committed exception through genuine paired finite Symbol jobs.
//! Selection, protected reads and DbgHelp run on the blocking execution boundary.
//! Results become available only after the original Main terminates and checked
//! Provider, reader and selected-file cleanup completes. No UI parsing occurs.
mod cause;
mod service;
mod source;
mod worker;
pub use cause::{SymbolReportCause, SymbolReportFailure};
pub use service::{SymbolReportObservation, SymbolReportResult, SymbolReportService};
use sugarsubstitute_contracts::incident::{DumpId, IncidentKey, IncidentRevision};
use sugarsubstitute_supervision::committed_reader::ProtectedCommittedReadRequest;
/// Own the genuine protected namespace loan and inert expected committed identities.
/// The native reader independently authenticates the original namespace and Core.
pub struct SymbolReportRequest {
    protected: ProtectedCommittedReadRequest,
    key: IncidentKey,
    revision: IncidentRevision,
    dump: DumpId,
}
impl SymbolReportRequest {
    /// Bind the actual borrowing request to the exact desired Summary and dump.
    /// No path, native handle or matching metadata becomes authority.
    pub const fn new(
        protected: ProtectedCommittedReadRequest,
        key: IncidentKey,
        revision: IncidentRevision,
        dump: DumpId,
    ) -> Self {
        Self { protected, key, revision, dump }
    }
}
