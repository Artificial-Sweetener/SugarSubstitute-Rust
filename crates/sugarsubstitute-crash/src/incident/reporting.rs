// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Commit report metadata without copying immutable payload generations.
use super::{IncidentCommit, IncidentRepository, IncidentStoreError, StoredIncidentSummary};
use sugarsubstitute_contracts::{execution::JobContext, incident::*};
use sugarsubstitute_supervision::diagnostic_storage::*;
#[path = "reporting/symbols.rs"]
mod symbols;
pub use symbols::SymbolRecordingError;

/// Constructor-private acknowledgement backed by actual checked summary activation.
/// The host authenticates the reporter separately before submitting its command.
pub struct AcknowledgementCommit {
    at: IncidentUtc,
    committed: IncidentCommit,
}
impl AcknowledgementCommit {
    /// Borrow the exact summary and its actual storage activation observation.
    pub const fn committed(&self) -> &IncidentCommit {
        &self.committed
    }
    /// Project inert acknowledgement data after actual durable activation or reconciliation.
    pub fn acknowledgement(&self) -> DurableAcknowledgement {
        DurableAcknowledgement::new(self.committed.key(), self.committed.revision(), self.at)
    }
}

impl IncidentRepository {
    /// Commit one observed report attempt at the exact current summary revision.
    ///
    /// The authenticated adapter supplies the observation; session matching data is not
    /// authentication. A failure can be recorded even when a referenced payload is absent.
    /// No method here schedules another reporter or changes payload availability facts.
    /// # Errors
    /// Stale/foreign bindings, exhausted counters, live storage intents and uncertain
    /// activation reject the commit while retaining native custody and original causes.
    pub fn record_attempt(
        &mut self,
        binding: ReportBinding,
        attempt: ReportAttempt,
        context: &JobContext,
    ) -> Result<IncidentCommit, IncidentStoreError> {
        let (stored, facts) = self.report_snapshot(binding, context)?;
        let current = stored.summary();
        let attempts = current
            .facts()
            .reporting()
            .attempts()
            .checked_add(1)
            .ok_or(IncidentStoreError::Capacity)?;
        let reporting = ReportDisposition::new(attempts, Some(attempt))?;
        let custody = self.storage.recovery_record(&self.operation, context)?;
        let updated = metadata_summary(
            &stored,
            current.revision().next()?,
            current.facts().retention(),
            reporting,
        )?;
        self.commit_report_metadata(
            super::allocation::RetainedSummary::new(updated, custody),
            facts,
            context,
        )
    }

    /// Durably acknowledge an explicit authenticated dismissal at its expected revision.
    ///
    /// The first acknowledgement time is immutable. Repeating dismissal at the current
    /// revision reconciles actual storage without creating another revision. A restart
    /// request remains matching intent; only the separate M1 owner may authorize launch.
    /// # Errors
    /// Copy/export actions, stale bindings, regressing first timestamps, live protected
    /// intents or uncertain read/activation/close cannot produce an acknowledgement proof.
    pub fn acknowledge(
        &mut self,
        command: ReportCommand,
        at: IncidentUtc,
        context: &JobContext,
    ) -> Result<AcknowledgementCommit, IncidentStoreError> {
        if !matches!(
            command.action(),
            ReportAction::Dismiss | ReportAction::DismissAndRequestRestart
        ) {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let (stored, facts) = self.report_snapshot(command.binding(), context)?;
        let current = stored.summary();
        let (committed, at) = if let Some(first) = current.facts().retention().acknowledged() {
            let actual = self.storage.reconcile_activation(
                DiagnosticObject::Summary(current.seed().key()),
                super::repository::predecessor(facts),
                &self.operation,
                context,
            )?;
            (IncidentCommit { stored, operation: self.operation.key(), actual }, first)
        } else {
            let retention = RetentionFacts::new(current.facts().retention().occurred(), Some(at))?;
            let custody = self.storage.recovery_record(&self.operation, context)?;
            let updated = metadata_summary(
                &stored,
                current.revision().next()?,
                retention,
                current.facts().reporting(),
            )?;
            (
                self.commit_report_metadata(
                    super::allocation::RetainedSummary::new(updated, custody),
                    facts,
                    context,
                )?,
                at,
            )
        };
        Ok(AcknowledgementCommit { committed, at })
    }

    fn report_snapshot(
        &mut self,
        binding: ReportBinding,
        context: &JobContext,
    ) -> Result<(super::allocation::RetainedSummary, DiagnosticReadFacts), IncidentStoreError> {
        if self.pending.is_some() || self.preflight.is_some() {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let (stored, facts) = self
            .find_summary(binding.incident(), context)?
            .ok_or(IncidentStoreError::MissingPayload)?;
        if stored.summary().revision() != binding.revision() {
            return Err(IncidentStoreError::StaleRevision);
        }
        Ok((stored, facts))
    }

    fn commit_report_metadata(
        &mut self,
        stored: super::allocation::RetainedSummary,
        previous: DiagnosticReadFacts,
        context: &JobContext,
    ) -> Result<IncidentCommit, IncidentStoreError> {
        let encoded = self.encode_summary(&stored, context)?;
        let actual = self.activate_bytes(
            DiagnosticObject::Summary(stored.summary().seed().key()),
            encoded.as_bytes(),
            super::repository::predecessor(previous),
            context,
        )?;
        Ok(IncidentCommit { stored, operation: self.operation.key(), actual })
    }
}

fn metadata_summary(
    previous: &StoredIncidentSummary,
    revision: IncidentRevision,
    retention: RetentionFacts,
    reporting: ReportDisposition,
) -> Result<StoredIncidentSummary, IncidentStoreError> {
    let current = previous.summary();
    let facts = current.facts();
    StoredIncidentSummary::new(
        IncidentSummary::new(
            current.seed().clone(),
            revision,
            SummaryFacts::new(
                retention,
                facts.exit(),
                facts.failure(),
                facts.capture(),
                facts.payload().clone(),
                reporting,
                facts.symbols().clone(),
            ),
        )?,
        previous.payload_revision(),
    )
}
