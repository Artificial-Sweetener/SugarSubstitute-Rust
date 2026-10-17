// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resume the same closed partial HTTP owner under its applicable finite probe interval.
use super::*;

impl ReadinessCampaign {
    /// Observe qualified partial progress without granting readiness or fresh input authority.
    /// Pending assets retain their existing retirement reserve; insufficient original
    /// time waits for the original cutoff rather than admitting an empty phase.
    /// # Errors
    /// Rejects clock regression or expiry of the unchanged original campaign.
    pub fn needs_continuation(&self, now: Moment) -> Result<bool, CampaignError> {
        self.validate_now(now)?;
        if self.terminal || self.ready || self.attempt.is_some() || self.cleanup.is_some() {
            return Ok(false);
        }
        let until = self.probe_until(now)?;
        Ok(self.owner.as_ref().is_some_and(|owner| owner.continuation_fits(now, until)))
    }

    /// Submit a finite phase against the identical source/listener/claim/cursor.
    /// Each phase remains bounded by two seconds and any applicable finite campaign cutoff.
    /// # Errors
    /// Rejects unavailable/foreign/cancelled scope, late or excessive deadlines,
    /// and phases too short to progress the retained asset cohort;
    /// preserves the same owner and closes admission on execution failure.
    pub fn submit_continuation(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<JobId, CampaignError> {
        let now = execution.now().map_err(|cause| {
            self.close_admission();
            CampaignError::Clock(cause)
        })?;
        self.observe_now(now)?;
        if self.readiness_scope.is_some_and(|id| id != scope.id()) {
            return Err(CampaignError::Rejected(CampaignRejection::InvalidSpecification));
        }
        if !self.needs_continuation(now)? {
            return Err(CampaignError::Rejected(CampaignRejection::WrongPhase));
        }
        self.validate_scope(scope).inspect_err(|_| self.terminal = true)?;
        let until = self.probe_until(now)?;
        if spec.deadline.is_none_or(|at| {
            at <= now
                || at > until
                || !self.owner.as_ref().is_some_and(|owner| owner.continuation_fits(now, at))
        }) {
            return Err(CampaignError::Rejected(CampaignRejection::InvalidSpecification));
        }
        let owner =
            self.owner.as_mut().ok_or(CampaignError::Rejected(CampaignRejection::WrongPhase))?;
        let id = owner.submit_continuation(execution, scope, spec).map_err(|cause| {
            self.terminal = true;
            CampaignError::Admission(cause)
        })?;
        self.attempt =
            Some(Attempt { job: id, scope: scope.id(), until: spec.deadline.unwrap_or(until) });
        self.last_now = now;
        Ok(id)
    }
}
