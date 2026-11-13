// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Allocate the existing store sequencer for one pre-ACK recovery origin journal.
use super::*;
impl IncidentCoordinator {
    /// Reserve distinct original store correlation without acknowledging the report.
    /// Native source/grant/profile custody and actual publication remain adapter obligations.
    /// # Errors
    /// Rejects stale/unpresented/refreshing, already-mutating, retired or acknowledged views,
    /// and preserves the operation highwater on rejection or exhaustion.
    pub fn admit_recovery_capture_origin(
        &mut self,
        binding: ReportBinding,
        context: IncidentRequestContext,
    ) -> Result<IncidentOperationScope, IncidentDecisionError> {
        let record = self.record(self.index(binding.incident())?)?;
        if record.binding != Some(binding) || record.revision() != binding.revision() {
            return Err(IncidentDecisionError::StaleOperation);
        }
        if !record.presented
            || record.refresh_pending
            || record.pending.is_some()
            || record.acknowledgement.is_some()
            || record.reporter_retired
            || record.summary.is_none()
        {
            return Err(IncidentDecisionError::WrongPhase);
        }
        let scope = self.scope(binding.incident(), binding.revision(), context.budget())?;
        self.next_operation = scope.operation().id().get();
        Ok(scope)
    }
}
