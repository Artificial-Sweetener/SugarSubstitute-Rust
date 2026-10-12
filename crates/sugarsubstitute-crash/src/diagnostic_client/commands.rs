// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Define closed diagnostic intents and passive reporter observations.
use super::*;
use sugarsubstitute_contracts::incident::*;
/// Closed publication phases under one original operation/helper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticPublication<'a> {
    /// Preflight actual incoming demand before staging.
    Begin {
        /// Exact expected Summary.
        expected: DiagnosticExpectedSummary,
        /// Prepared bounded demand.
        demand: DiagnosticPublicationDemand,
        /// Canonical bounded original seed bytes.
        seed: &'a [u8],
    },
    /// One contiguous raw attachment/summary transfer.
    Chunk(DiagnosticChunk<'a>),
    /// Complete canonical Summary digest after all admitted bytes.
    Finish {
        /// Complete bounded Summary bytes.
        bytes: u64,
        /// Exact digest.
        sha256: [u8; 32],
    },
}
/// Native selection data; actual destination authority belongs to Storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticSelection<'a> {
    /// Positive exact native selection identity.
    pub id: DiagnosticRequestId,
    /// Explicit export content consent.
    pub selection: ExportSelection,
    /// Exact selected overwrite consent, not existing-target proof.
    pub overwrite: bool,
    /// Little-endian UTF16 units, <=1024 and no embedded NUL.
    pub destination: &'a [u8],
}
/// Closed effects; none execute repository/native operations during decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticCommand<'a> {
    /// Bounded original result facts; authenticated matching is not durable authority.
    Result {
        /// Exact closed original command kind.
        kind: DiagnosticRequestKind,
        /// Digest of original complete request bytes.
        request_sha256: [u8; 32],
        /// Original bounded terminal facts.
        reply: DiagnosticReply,
    },
    /// Resolve only trusted inherited root input; there is no wire root path.
    OpenProtectedRoot,
    /// Enumerate under current admitted recovery operation.
    Recover,
    /// Strict seed codec is subsequently consumed by the actual repository.
    Seed(&'a [u8]),
    /// Sole publication transaction phases, never a rebind.
    Publish(DiagnosticPublication<'a>),
    /// Request exact protected committed content.
    Load,
    /// Open/pin actual export inputs and selected destination.
    RetainExport(DiagnosticSelection<'a>),
    /// One bounded export transfer under original scope.
    ExportChunk(DiagnosticChunk<'a>),
    /// Checked export activation/retirement result is separate.
    FinishExport([u8; 32]),
    /// Explicit dismissal intent, with separately checked durable completion.
    Acknowledge {
        /// Exact reporter session.
        session: ReportSessionId,
        /// Forward restart intent only after actual settlement.
        restart: bool,
    },
    /// Persist actual presentation/failure outcome once.
    RecordAttempt {
        /// Exact reporter session.
        session: ReportSessionId,
        /// Categorical observed attempt.
        attempt: ReportAttempt,
    },
    /// Request actual dump read lease; UUID is inert.
    RetainDump(DumpId),
    /// Request exact release association; digest grants no publisher trust.
    Symbolize {
        /// Original captured UUID.
        dump: DumpId,
        /// Exact release annotation digest.
        release: [u8; 32],
    },
    /// Request actual source disposition under retained owner authority.
    DiscardCapture(DumpId),
    /// Close admission and retire actual owners; success is not implied.
    Retire,
    /// Start one complete bounded redacted view transfer.
    BeginView {
        /// Reporter session.
        session: ReportSessionId,
        /// Total redacted UTF8 bytes.
        total: u64,
        /// Complete digest.
        sha256: [u8; 32],
    },
    /// Ordered redacted-view raw chunk.
    ViewChunk {
        /// Reporter session.
        session: ReportSessionId,
        /// Original transfer descriptor.
        chunk: DiagnosticChunk<'a>,
    },
    /// Finish only the exact complete transfer.
    EndView {
        /// Reporter session.
        session: ReportSessionId,
        /// Complete digest.
        sha256: [u8; 32],
    },
    /// Begin replacement in the same session after an actual CAS.
    ReplaceView {
        /// Same reporter session.
        session: ReportSessionId,
        /// Exact previous visible revision.
        old: IncidentRevision,
        /// New total redacted UTF8 bytes.
        total: u64,
        /// Complete digest.
        sha256: [u8; 32],
    },
    /// Actual native presentation claim; host authenticates origin.
    Presented(ReportSessionId),
    /// Actual native replacement claim; host authenticates exact current view.
    ViewReplaced(ReportSessionId),
    /// Inert user action at exact currently authenticated view.
    Action {
        /// Reporter session.
        session: ReportSessionId,
        /// Closed user intent.
        action: ReportAction,
    },
    /// Actual native selected bounded destination, not a file lease.
    SaveSelectionReady {
        /// Reporter session.
        session: ReportSessionId,
        /// Exact consent/selection.
        selection: DiagnosticSelection<'a>,
    },
    /// Cancellation changes neither acknowledgement nor retention.
    SaveSelectionCancelled(ReportSessionId),
    /// Native UI retirement claim, not process/family/pipe settlement.
    NativeRetired(ReportSessionId),
    /// Authorize UI close only after host's separately checked durable commit.
    CloseAfterDurableAck {
        /// Exact session.
        session: ReportSessionId,
        /// Inert matching durable acknowledgement data.
        acknowledgement: DurableAcknowledgement,
    },
    /// Original native copy request result, never acknowledgement.
    CopyResult {
        /// Exact session.
        session: ReportSessionId,
        /// Actual categorical outcome.
        result: DiagnosticActionResult,
    },
    /// Selected export outcome, never acknowledgement.
    ExportSelectionResult {
        /// Exact session.
        session: ReportSessionId,
        /// Actual categorical outcome.
        result: DiagnosticActionResult,
    },
    /// Actual categorical reporter failure, never automatic relaunch.
    ReporterFailed {
        /// Exact reporter session.
        session: ReportSessionId,
        /// Original categorical failure.
        failure: ReportFailure,
    },
}

/// Categorical result; original causes remain adapter-owned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticActionResult {
    /// Requested action actually completed.
    Success,
    /// Actual categorical failure.
    Failed(ReportFailure),
}
