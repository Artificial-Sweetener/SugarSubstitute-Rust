// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded exact expected mutations and independent custody obligations.
use super::*;
use crate::supervision::FailedDesktopFact;
use sugarsubstitute_contracts::incident::*;

#[derive(Clone, Debug, Eq, PartialEq)]
/// Accepted original lifecycle attribution and fault mechanism.
pub(super) struct FailureInput {
    pub fact: Option<FailedDesktopFact>,
    pub attribution: ExitAttribution,
    pub failure: FailureEvidence,
}
#[derive(Clone, Debug, Eq, PartialEq)]
/// One exact outstanding use case and its already-prepared successor content.
pub(super) enum Pending {
    Seed,
    Collection,
    Summary(IncidentSummary),
    Attempt {
        binding: ReportBinding,
        expected: IncidentSummary,
        attempt: ReportAttempt,
        at: IncidentUtc,
    },
    Acknowledgement {
        binding: ReportBinding,
        expected: IncidentSummary,
        at: IncidentUtc,
    },
    Export(ReportBinding),
}
#[derive(Clone, Debug, Eq, PartialEq)]
/// One bounded completed CAS correlation for exact duplicate observation handling.
pub(super) struct MutationCommit {
    pub scope: IncidentOperationScope,
    pub binding: ReportBinding,
    pub summary: IncidentSummary,
}
/// One preflighted subsequent failure CAS waiting for the preceding actual mutation.
pub(super) struct QueuedAttempt {
    pub failure: ReportFailure,
    pub context: IncidentRequestContext,
    pub observed_binding: ReportBinding,
    pub binding: ReportBinding,
    pub scope: IncidentOperationScope,
    pub expected: IncidentSummary,
}
/// One bounded retained incident and all independently observed custody obligations.
pub(super) struct Record {
    pub seed: IncidentSeed,
    pub seed_context: Option<IncidentRequestContext>,
    pub seed_scope: Option<IncidentOperationScope>,
    pub seed_committed: bool,
    pub original_publication: LogicalStoreOperationKey,
    pub source_scope: IncidentSourceScope,
    pub current: Option<IncidentOperationScope>,
    pub pending: Option<Pending>,
    pub failure: Option<FailureInput>,
    pub failure_context: Option<IncidentRequestContext>,
    pub summary: Option<IncidentSummary>,
    pub recovery_projection: Option<IncidentSummary>,
    pub published_scope: Option<IncidentOperationScope>,
    pub last_commit: Option<MutationCommit>,
    pub phase: IncidentServicePhase,
    pub binding: Option<ReportBinding>,
    pub automatic_report_attempted: bool,
    pub presented: bool,
    pub attempted_session: bool,
    pub last_attempt: Option<(ReportAttempt, IncidentRequestContext)>,
    pub queued_failure: Option<QueuedAttempt>,
    pub refresh_pending: bool,
    pub reporter_retired: bool,
    pub sources_retired: bool,
    pub source_requested: bool,
    pub source_failed: bool,
    pub family_settled: bool,
    pub clean_retirement: bool,
    pub acknowledgement: Option<DurableAcknowledgement>,
    pub restart_requested: bool,
    pub restart_forwarded: bool,
    pub first_failure: Option<IncidentServiceFailure>,
}
impl Record {
    /// Return the actually committed revision without advancing it.
    pub(super) fn revision(&self) -> IncidentRevision {
        self.summary.as_ref().map_or(self.seed.revision(), IncidentSummary::revision)
    }
    /// Require every independent acknowledgement and custody obligation before reuse.
    pub(super) fn reclaimable(&self) -> bool {
        self.pending.is_none()
            && self.queued_failure.is_none()
            && !self.refresh_pending
            && self.current.is_none()
            && if self.failure.is_none() && self.summary.is_none() {
                self.clean_retirement && self.sources_retired
            } else {
                self.acknowledgement.is_some()
                    && self.sources_retired
                    && self.family_settled
                    && self.reporter_retired
            }
    }
    /// Project retained matching facts without transferring native authority.
    pub(super) fn snapshot(&self) -> IncidentServiceSnapshot {
        IncidentServiceSnapshot {
            key: self.seed.key(),
            revision: self.revision(),
            phase: self.phase,
            original_publication: Some(self.original_publication),
            current_operation: self.current,
            source_scope: Some(self.source_scope),
            report_binding: self.binding,
            acknowledgement: self.acknowledgement,
            automatic_report_attempted: self.automatic_report_attempted,
            sources_retired: self.sources_retired,
            family_settled: self.family_settled,
            reporter_retired: self.reporter_retired,
            first_failure: self.first_failure,
        }
    }
    /// Keep the original first failure while leaving actual custody retained.
    pub(super) fn latch_failure(&mut self, failure: IncidentServiceFailure) {
        if self.first_failure.is_none() {
            self.first_failure = Some(failure);
        }
        self.phase = IncidentServicePhase::Failed;
    }
}
