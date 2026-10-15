// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume actual authenticated host projections, never manufacture adapter proofs.
use super::{
    CollectedIncidentEvidence, IncidentOperationScope, IncidentRecoveryScope,
    IncidentRequestContext, IncidentSourceScope,
};
use sugarsubstitute_contracts::{incident::*, lifecycle::RunKey};

/// Categorical failed use case; original native causes remain with its adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncidentServiceFailure {
    /// Seed could not be committed.
    SeedPublication,
    /// Evidence collection failed.
    Collection,
    /// Summary could not be committed.
    SummaryPublication,
    /// Actual report disposition could not be committed.
    AttemptPublication,
    /// Explicit acknowledgement could not be committed.
    Acknowledgement,
    /// Export failed without acknowledging.
    Export,
    /// Source retirement remains uncertain.
    SourceRetirement,
    /// Recovery failed under its original admission.
    Recovery,
}
/// Plain observations require matching actual private proofs in the composing host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IncidentServiceObservation {
    /// Actual seed activation and checked readback completed.
    SeedCommitted {
        /// Original scope.
        scope: IncidentOperationScope,
        /// Exact actually committed seed.
        seed: IncidentSeed,
    },
    /// Actual collection finished with explicit payload/symbol outcomes.
    EvidenceCollected {
        /// Original scope.
        scope: IncidentOperationScope,
        /// Actual collected facts.
        evidence: CollectedIncidentEvidence,
    },
    /// Actual summary activation and checked readback completed.
    SummaryCommitted {
        /// Original scope.
        scope: IncidentOperationScope,
        /// Exact actually committed summary.
        summary: IncidentSummary,
    },
    /// Actual checked symbol metadata activation completed before initial presentation.
    SymbolsCommitted {
        /// Exact original metadata mutation scope.
        scope: IncidentOperationScope,
        /// Original unpresented binding.
        binding: ReportBinding,
        /// Actual successor preserving original evidence and reporting facts.
        summary: IncidentSummary,
    },
    /// The host proved the exact symbol metadata operation never began.
    /// Original summary bytes and their session remain authoritative.
    SymbolsNotWritten {
        /// Same original mutation scope and logical operation.
        scope: IncidentOperationScope,
        /// Same unpresented binding; no successor was activated.
        binding: ReportBinding,
    },
    /// Native adapter actually presented the outstanding session.
    ReporterPresented {
        /// Exact outstanding session.
        binding: ReportBinding,
        /// Original request interval and actual UTC.
        context: IncidentRequestContext,
    },
    /// Actual report disposition CAS completed.
    AttemptCommitted {
        /// Exact mutation scope.
        scope: IncidentOperationScope,
        /// Previous binding used for this CAS.
        binding: ReportBinding,
        /// Actual successor content.
        summary: IncidentSummary,
    },
    /// Native UI actually consumed the new same-session binding.
    ReportViewReplaced {
        /// Exact new binding.
        binding: ReportBinding,
    },
    /// Actual acknowledgement CAS completed with exact UTC and content.
    Acknowledged {
        /// Exact mutation scope.
        scope: IncidentOperationScope,
        /// Previously visible binding.
        binding: ReportBinding,
        /// Actual durable acknowledgement.
        acknowledgement: DurableAcknowledgement,
        /// Actual successor content.
        summary: IncidentSummary,
    },
    /// Actual selected export finished without changing retention.
    ExportFinished {
        /// Original export scope.
        scope: IncidentOperationScope,
        /// Exact requesting binding.
        binding: ReportBinding,
    },
    /// Original pending use case failed, retaining its correlation for reconciliation.
    Failed {
        /// Exact pending scope.
        scope: IncidentOperationScope,
        /// Actual categorical failure.
        failure: IncidentServiceFailure,
    },
    /// Actual reporter failed; no new reporter or incident is admitted.
    ReporterFailed {
        /// Exact outstanding reporter.
        binding: ReportBinding,
        /// Actual failure category.
        failure: ReportFailure,
        /// Original request interval and actual UTC.
        context: IncidentRequestContext,
    },
    /// Actual reporter family and UI teardown completed.
    ReporterRetired {
        /// Exact retained reporter session.
        binding: ReportBinding,
    },
    /// Source cleanup failed while exact live or recovered custody remains retained.
    SourceRetirementFailed {
        /// Exact already-issued live or recovered source correlation.
        scope: IncidentSourceScope,
    },
    /// Actual separately retained source custody was settled.
    SourcesRetired {
        /// Exact already-issued source correlation.
        scope: IncidentSourceScope,
    },
    /// Clean seed removal and original owner settlement completed.
    SeedRetired {
        /// Exact live original seed scope.
        scope: IncidentOperationScope,
    },
    /// Actual native desktop family settled independently.
    FamilySettled {
        /// Exact original run.
        run: RunKey,
    },
    /// Actual independently retained supervisor family settled in full.
    SupervisorFamilySettled {
        /// Exact historical supervisor lifetime.
        owner: sugarsubstitute_contracts::lifecycle::CoordinatorKey,
    },
    /// Bounded validated store projection of one historical summary.
    Recovered {
        /// Current admitted recovery scope.
        scope: IncidentRecoveryScope,
        /// Actual recorded historical operation, absent for canonical summaries which persist none.
        original_operation: Option<LogicalStoreOperationKey>,
        /// Actual stored content.
        summary: IncidentSummary,
    },
    /// External observer proved an abandoned supervisor seed failed.
    RecoveredSupervisorSeed {
        /// Current admitted recovery scope.
        scope: IncidentRecoveryScope,
        /// Inert original publication identity.
        original_operation: LogicalStoreOperationKey,
        /// Actual stored seed.
        seed: IncidentSeed,
        /// External actual native status.
        actual_status: i64,
        /// Actual externally established failure mechanism.
        failure: FailureEvidence,
    },
    /// Current recovery failed while its original admission remains retained.
    RecoveryFailed {
        /// Exact current original recovery scope.
        scope: IncidentRecoveryScope,
    },
    /// Actual bounded recovery enumeration finished; new projections are closed.
    RecoveryFinished {
        /// Exact current recovery scope.
        scope: IncidentRecoveryScope,
    },
}
