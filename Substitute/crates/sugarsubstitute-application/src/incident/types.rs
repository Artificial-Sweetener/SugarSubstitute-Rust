// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expose bounded service state and independently supplied collection outcomes.
use super::{IncidentOperationScope, IncidentSourceScope};
use sugarsubstitute_contracts::incident::*;

/// Maximum retained live or recovered incident records.
pub const MAX_INCIDENT_RECORDS: usize = 128;
/// Maximum simultaneously emitted policy effects per transition.
pub const MAX_INCIDENT_EFFECTS: usize = 8;

/// Inspect the one current recovery admission and its independently latched outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IncidentRecoverySnapshot {
    /// Original current operation and immutable monotonic interval.
    pub scope: super::IncidentRecoveryScope,
    /// Actual enumeration completion closed new projection admission.
    pub finished: bool,
    /// Original categorical failure remains visible after late actual completion.
    pub first_failure: Option<super::IncidentServiceFailure>,
}

/// Collected facts cannot change the original seed, attribution, or supplied fault.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectedIncidentEvidence {
    occurred: IncidentUtc,
    capture: CaptureEvidence,
    payload: PayloadDisposition,
    symbols: SymbolOutcome,
}
impl CollectedIncidentEvidence {
    /// Combine actual adapter outcomes; summary construction validates related facts.
    pub const fn new(
        occurred: IncidentUtc,
        capture: CaptureEvidence,
        payload: PayloadDisposition,
        symbols: SymbolOutcome,
    ) -> Self {
        Self { occurred, capture, payload, symbols }
    }
    /// Return actual occurrence UTC.
    pub const fn occurred(&self) -> IncidentUtc {
        self.occurred
    }
    /// Return the actual capture disposition.
    pub const fn capture(&self) -> CaptureEvidence {
        self.capture
    }
    /// Borrow payload retention facts.
    pub const fn payload(&self) -> &PayloadDisposition {
        &self.payload
    }
    /// Borrow the actual symbol result or explicit unresolved outcome.
    pub const fn symbols(&self) -> &SymbolOutcome {
        &self.symbols
    }
}

/// Observe service progress without inferring native settlement from cancellation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncidentServicePhase {
    /// Seed publication is outstanding.
    SeedPublishing,
    /// A seed was actually committed; no failure is admitted.
    SeedCommitted,
    /// Original accepted failure evidence is being collected.
    Collecting,
    /// Exact summary publication is outstanding.
    SummaryPublishing,
    /// Committed incident has no current visible report.
    Pending,
    /// Report presentation or its current view is retained.
    Reporting,
    /// Durable acknowledgement is outstanding.
    Acknowledging,
    /// Actual durable acknowledgement was accepted.
    Acknowledged,
    /// Clean seed retirement is outstanding.
    Retiring,
    /// All separately required obligations were settled.
    Settled,
    /// A categorical failure is latched; original pending correlation remains.
    Failed,
}
/// Inspect retained correlation and independent obligations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IncidentServiceSnapshot {
    /// Exact full subject.
    pub key: IncidentKey,
    /// Current committed revision, or immutable seed revision before first commit.
    pub revision: IncidentRevision,
    /// Actual current service phase.
    pub phase: IncidentServicePhase,
    /// Original publication identity, including historical epoch where applicable.
    pub original_publication: Option<LogicalStoreOperationKey>,
    /// Outstanding current mutation/collection correlation.
    pub current_operation: Option<IncidentOperationScope>,
    /// Current live or recovered source correlation.
    pub source_scope: Option<IncidentSourceScope>,
    /// Current reporter session and expected view revision.
    pub report_binding: Option<ReportBinding>,
    /// Accepted actual durable acknowledgement.
    pub acknowledgement: Option<DurableAcknowledgement>,
    /// One ordinary-launch automatic reporter was already requested.
    pub automatic_report_attempted: bool,
    /// Actual source retirement has been observed.
    pub sources_retired: bool,
    /// Actual family settlement has been observed independently.
    pub family_settled: bool,
    /// Actual reporter retirement has been observed independently.
    pub reporter_retired: bool,
    /// Original first categorical failure remains unchanged.
    pub first_failure: Option<super::IncidentServiceFailure>,
}
