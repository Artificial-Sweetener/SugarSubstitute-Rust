// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Commit a genuine retired Symbol observation without changing original failure or payload facts.
use super::*;
use sugarsubstitute_contracts::execution::AdmissionError;
/// Preserve the real sealed-origin rejection or original repository failure.
#[derive(Debug)]
pub enum SymbolRecordingError {
    /// The retained real original Main context was unavailable.
    Origin(AdmissionError),
    /// The actual protected repository rejected or failed metadata publication.
    Store(IncidentStoreError),
    /// Metadata work disagreed with the sealed owner/cutoff or current execution controls.
    Scope,
}
impl std::fmt::Display for SymbolRecordingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Symbol metadata recording failed")
    }
}
impl std::error::Error for SymbolRecordingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Origin(cause) => Some(cause),
            Self::Store(cause) => Some(cause),
            Self::Scope => None,
        }
    }
}
impl IncidentRepository {
    /// Attach only the genuine checked Symbol service observation at the exact current binding.
    /// Original failure, capture, payload generation, retention and reporting facts survive.
    /// # Errors
    /// Preserves actual origin/storage errors; rejects foreign execution, renewed/expired work,
    /// stale incident/revision/dump/build, live intent or uncertain activation without a commit.
    pub fn record_symbols(
        &mut self,
        binding: ReportBinding,
        observation: &super::super::SymbolReportObservation,
        context: &JobContext,
    ) -> Result<IncidentCommit, SymbolRecordingError> {
        let (owner, until) =
            observation.original_admission().map_err(SymbolRecordingError::Origin)?;
        if context.owner_id() != owner
            || context.cancellation_requested()
            || context
                .deadline()
                .is_none_or(|deadline| deadline > until || context.now() >= deadline)
            || context.now() >= until
        {
            return Err(SymbolRecordingError::Scope);
        }
        self.record_symbol_metadata(binding, observation.identity(), observation.frame(), context)
            .map_err(SymbolRecordingError::Store)
    }
    /// Apply the repository algorithm at its sealed public boundary; tests supply inert frame facts.
    fn record_symbol_metadata(
        &mut self,
        binding: ReportBinding,
        identity: (IncidentKey, IncidentRevision, DumpId),
        frame: &FailingFrame,
        context: &JobContext,
    ) -> Result<IncidentCommit, IncidentStoreError> {
        let (stored, previous) = self.report_snapshot(binding, context)?;
        let current = stored.summary();
        let (key, revision, dump) = identity;
        if (key, revision) != (binding.incident(), binding.revision())
            || current.facts().capture() != CaptureEvidence::Finished(dump)
            || frame.dump() != dump
            || frame.release().build() != current.seed().build()
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let custody = self.storage.recovery_record(&self.operation, context)?;
        let facts = current.facts();
        let updated = StoredIncidentSummary::new(
            IncidentSummary::new(
                current.seed().clone(),
                current.revision().next()?,
                SummaryFacts::new(
                    facts.retention(),
                    facts.exit(),
                    facts.failure(),
                    facts.capture(),
                    facts.payload().clone(),
                    facts.reporting(),
                    SymbolOutcome::Resolved(frame.clone()),
                ),
            )?,
            stored.payload_revision(),
        )?;
        self.commit_report_metadata(
            super::super::allocation::RetainedSummary::new(updated, custody),
            previous,
            context,
        )
    }
}
#[cfg(test)]
#[path = "symbols/tests.rs"]
mod tests;
