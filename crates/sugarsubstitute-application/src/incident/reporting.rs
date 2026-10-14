// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serialize exact report mutations and fence actions until actual view refresh.
mod attempts;
mod queued;
mod symbols;

use super::{
    state::{MutationCommit, Pending, Record},
    *,
};
use sugarsubstitute_contracts::incident::*;

/// Prepare a successor that preserves all immutable evidence and seed facts.
pub(super) fn rewritten(
    record: &Record,
    reporting: ReportDisposition,
    ack: Option<IncidentUtc>,
) -> Result<IncidentSummary, IncidentDecisionError> {
    let summary = record.summary.as_ref().ok_or(IncidentDecisionError::WrongPhase)?;
    let facts = summary.facts();
    Ok(IncidentSummary::new(
        summary.seed().clone(),
        summary.revision().next()?,
        SummaryFacts::new(
            RetentionFacts::new(facts.retention().occurred(), ack)?,
            facts.exit(),
            facts.failure(),
            facts.capture(),
            facts.payload().clone(),
            reporting,
            facts.symbols().clone(),
        ),
    )?)
}
impl IncidentCoordinator {
    /// Allocate correlation for reading the exact retained report revision.
    /// The scope grants no storage authority and leaves mutation and presentation state intact.
    pub fn admit_report_view(
        &mut self,
        binding: ReportBinding,
        context: IncidentRequestContext,
    ) -> Result<IncidentOperationScope, IncidentDecisionError> {
        let record = self.record(self.index(binding.incident())?)?;
        if record.binding != Some(binding) || record.revision() != binding.revision() {
            return Err(IncidentDecisionError::StaleOperation);
        }
        let read_allowed = match &record.pending {
            None => true,
            Some(Pending::Export(original)) => *original == binding,
            _ => false,
        };
        if record.reporter_retired || record.summary.is_none() || !read_allowed {
            return Err(IncidentDecisionError::WrongPhase);
        }
        let scope = self.scope(binding.incident(), binding.revision(), context.budget())?;
        self.next_operation = scope.operation().id().get();
        Ok(scope)
    }
    /// Validate the exact visible view and admit only its requested use case.
    /// Copy allocates no operation. Export never acknowledges. Dismiss closes only
    /// after the exact actual durable successor is independently observed.
    pub fn report_action(
        &mut self,
        command: ReportCommand,
        context: IncidentRequestContext,
    ) -> Result<IncidentReportDecision, IncidentDecisionError> {
        let binding = command.binding();
        let index = self.index(binding.incident())?;
        let record = self.record(index)?;
        if record.binding != Some(binding) {
            return Err(IncidentDecisionError::StaleOperation);
        }
        if !record.presented
            || record.refresh_pending
            || record.pending.is_some()
            || record.acknowledgement.is_some()
            || record.reporter_retired
        {
            return Err(IncidentDecisionError::WrongPhase);
        }
        if command.action() == ReportAction::CopyText {
            return Ok(IncidentReportDecision::CopyText { binding });
        }
        let scope = self.scope(binding.incident(), record.revision(), context.budget())?;
        let (pending, effects, restart) = match command.action() {
            ReportAction::Export(selection) => (
                Pending::Export(binding),
                IncidentEffects::one(IncidentEffect::Export { scope, binding, selection }),
                false,
            ),
            ReportAction::Dismiss | ReportAction::DismissAndRequestRestart => {
                let summary = record.summary.as_ref().ok_or(IncidentDecisionError::WrongPhase)?;
                let expected = rewritten(record, summary.facts().reporting(), Some(context.at()))?;
                (
                    Pending::Acknowledgement { binding, expected, at: context.at() },
                    IncidentEffects::one(IncidentEffect::Acknowledge {
                        scope,
                        binding,
                        at: context.at(),
                    }),
                    command.action() == ReportAction::DismissAndRequestRestart,
                )
            }
            ReportAction::CopyText => return Err(IncidentDecisionError::WrongPhase),
        };
        self.next_operation = scope.operation().id().get();
        let record = self.record_mut(index)?;
        record.pending = Some(pending);
        record.current = Some(scope);
        if matches!(
            command.action(),
            ReportAction::Dismiss | ReportAction::DismissAndRequestRestart
        ) {
            record.phase = IncidentServicePhase::Acknowledging;
            record.restart_requested = restart;
        }
        Ok(IncidentReportDecision::Effects(effects))
    }
    /// Close only after exact actual durable acknowledgement content and UTC match.
    pub(super) fn acknowledged(
        &mut self,
        scope: IncidentOperationScope,
        binding: ReportBinding,
        ack: DurableAcknowledgement,
        summary: IncidentSummary,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let index = self.index(scope.incident())?;
        let record = self.record(index)?;
        let commit = MutationCommit { scope, binding, summary: summary.clone() };
        if record.last_commit.as_ref() == Some(&commit) && record.acknowledgement == Some(ack) {
            return Ok(IncidentEffects::default());
        }
        if record.current != Some(scope) {
            return Err(IncidentDecisionError::StaleOperation);
        }
        let Some(Pending::Acknowledgement { binding: original, expected, at }) = &record.pending
        else {
            return Err(IncidentDecisionError::WrongPhase);
        };
        if *original != binding
            || *expected != summary
            || ack != DurableAcknowledgement::new(scope.incident(), summary.revision(), *at)
        {
            return Err(IncidentDecisionError::MismatchedCommit);
        }
        let restart = record.restart_requested
            && record.sources_retired
            && record.family_settled
            && !record.restart_forwarded;
        let restart_run = match record.seed.key().subject() {
            IncidentSubject::Desktop(run) if restart => Some(run),
            _ => None,
        };
        let record = self.record_mut(index)?;
        record.summary = Some(summary);
        record.acknowledgement = Some(ack);
        record.pending = None;
        record.current = None;
        record.last_commit = Some(commit);
        record.phase = IncidentServicePhase::Acknowledged;
        record.restart_forwarded =
            restart && matches!(record.seed.key().subject(), IncidentSubject::Desktop(_));
        let close = IncidentEffect::CloseAfterDurableAck { binding, acknowledgement: ack };
        let queued = self.activate_queued_attempt(index)?;
        let effects = match (restart_run, queued) {
            (Some(run), Some(effect)) => IncidentEffects::three(
                close,
                IncidentEffect::ForwardRestartIntent { run, acknowledgement: ack },
                effect,
            ),
            (Some(run), None) => IncidentEffects::pair(
                close,
                IncidentEffect::ForwardRestartIntent { run, acknowledgement: ack },
            ),
            (None, Some(effect)) => IncidentEffects::pair(close, effect),
            (None, None) => IncidentEffects::one(close),
        };
        Ok(effects)
    }
    /// Finish the exact selected export without acknowledging or revising the incident.
    pub(super) fn export_finished(
        &mut self,
        scope: IncidentOperationScope,
        binding: ReportBinding,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let index = self.index(scope.incident())?;
        let record = self.record(index)?;
        if record.current != Some(scope) {
            return Err(IncidentDecisionError::StaleOperation);
        }
        if record.pending != Some(Pending::Export(binding)) {
            return Err(IncidentDecisionError::WrongPhase);
        }
        let record = self.record_mut(index)?;
        record.current = None;
        record.pending = None;
        if let Some(effect) = self.activate_queued_attempt(index)? {
            return Ok(IncidentEffects::one(effect));
        }
        Ok(IncidentEffects::default())
    }
}
