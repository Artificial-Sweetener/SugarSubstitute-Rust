// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain only genuine sealed Symbol results and metadata commits at the same original cutoff.
use super::*;
use std::sync::Arc;
/// Own one genuine observation allocation under the same actual fixed source claim.
/// The allocation drops before its claim; no frame or original provider charge is split.
pub(in crate::production) struct ReportingSymbols {
    observation: Box<SymbolReportObservation>,
    _metadata: Arc<MemoryClaim>,
}
impl ReportingSymbols {
    /// Borrow the complete sealed observation while its original source claim remains held.
    pub(in crate::production) fn observation(&self) -> &SymbolReportObservation {
        &self.observation
    }
    /// Allocate only after Source retains its sole existing role claim outside the lock.
    pub(super) fn new(observation: SymbolReportObservation, metadata: Arc<MemoryClaim>) -> Self {
        Self { observation: Box::new(observation), _metadata: metadata }
    }
    /// Return the entire original sealed observation during checked passive input restoration.
    /// Its source/provider charge stays inseparable; this frees the bounded metadata allocation.
    pub(in crate::production) fn restore(self) -> SymbolReportObservation {
        *self.observation
    }
}
/// Verify original actual owner/effective deadline before opening any protected reporting storage.
pub(super) fn validate(input: &ReportingInput, context: &JobContext) -> Result<(), ReportingCause> {
    let ReportingWork::Symbols { result, .. } = &input.work else {
        return Ok(());
    };
    let (owner, until) = result
        .observation()
        .original_admission()
        .map_err(|cause| ReportingCause::Symbols(SymbolRecordingError::Origin(cause)))?;
    if context.owner_id() != owner
        || context.cancellation_requested()
        || context.deadline().is_none_or(|deadline| deadline > until || context.now() >= deadline)
        || context.now() >= until
    {
        return Err(ReportingCause::Symbols(SymbolRecordingError::Scope));
    }
    Ok(())
}
/// Record using disjoint owned destinations while the same original sealed input remains borrowed.
pub(super) fn record(
    repository: &mut Option<IncidentRepository>,
    output: &mut Option<IncidentCommit>,
    binding: ReportBinding,
    observation: &SymbolReportObservation,
    context: &JobContext,
) -> Result<(), ReportingCause> {
    let repository = repository.as_mut().ok_or(ReportingCause::Identity)?;
    *output = Some(
        repository
            .record_symbols(binding, observation, context)
            .map_err(ReportingCause::Symbols)?,
    );
    Ok(())
}
impl ReportingOperation {
    /// Borrow genuine checked symbol metadata activation while original input and completion remain.
    pub(in crate::production) fn symbols(&self) -> Option<&IncidentCommit> {
        self.symbols.as_ref()
    }
}
