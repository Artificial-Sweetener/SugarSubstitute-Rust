// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Separate finite preparation/probes from an explicitly admitted live managed wait.
use super::*;
use std::time::Duration;

impl ReadinessCampaign {
    /// Admit readiness waiting in the host's actual cancellable application scope.
    ///
    /// This grants no process, listener, source or Ready authority. The host first
    /// completes finite preparation and retains cancellation; managed targets also
    /// keep their matching actual process exit watch. Each probe remains finite.
    /// Replaying the same scope preserves the immutable original preparation budget.
    ///
    /// # Errors
    /// Rejects finite/closed/cancelled or foreign scopes, terminal/pending work,
    /// regression and first admission at or after the preparation cutoff.
    pub fn admit_readiness_wait(
        &mut self,
        scope: &Scope,
        now: Moment,
    ) -> Result<(), CampaignError> {
        self.validate_now(now)?;
        self.validate_scope(scope)?;
        if scope.spec().deadline.is_some() {
            return Err(CampaignError::Rejected(CampaignRejection::InvalidSpecification));
        }
        if self.terminal {
            return Err(CampaignError::Rejected(CampaignRejection::WrongPhase));
        }
        if let Some(original) = self.readiness_scope {
            return if original == scope.id() {
                Ok(())
            } else {
                Err(CampaignError::Rejected(CampaignRejection::InvalidSpecification))
            };
        }
        if self.ready || self.owner.is_some() || self.attempt.is_some() || self.cleanup.is_some() {
            return Err(CampaignError::Rejected(CampaignRejection::WrongPhase));
        }
        self.budget.probe_until(now).map_err(CampaignError::Budget)?;
        self.readiness_scope = Some(scope.id());
        self.execution = Some(scope.owner_id());
        self.last_now = now;
        Ok(())
    }

    /// Return the original finite cutoff, or no aggregate cutoff after readiness-wait admission.
    pub const fn campaign_until(&self) -> Option<Moment> {
        if self.readiness_scope.is_some() { None } else { Some(self.budget.until()) }
    }

    /// Bound each actual probe by two seconds and any still-applicable finite cutoff.
    ///
    /// # Errors
    /// Preserves original clock/expiry causes; admitted readiness probes still reject
    /// regression and checked addition overflow rather than renewing an old job.
    pub fn probe_until(&self, now: Moment) -> Result<Moment, CampaignError> {
        self.validate_now(now)?;
        if self.readiness_scope.is_some() {
            self.budget.readiness_probe_until(now)
        } else {
            self.budget.probe_until(now)
        }
        .map_err(CampaignError::Budget)
    }

    /// Return the first retained cleanup cutoff for the current probe's actual outputs.
    ///
    /// Final cancellation uses its original shutdown settle deadline. A live
    /// readiness retry freezes at most fifteen seconds at terminal-result adoption;
    /// finite campaigns retain their original cutoff. Later admission cannot renew it.
    ///
    /// # Errors
    /// Rejects regressing clocks or an unrepresentable fresh cleanup deadline.
    pub fn cleanup_deadline(&self, now: Moment) -> Result<Moment, CampaignError> {
        self.validate_now(now)?;
        if let Some(until) = self.cleanup_until {
            return Ok(until);
        }
        if let Some(shutdown) = self.shutdown {
            return Ok(shutdown.settle_until);
        }
        if let Some(until) = self.campaign_until() {
            return Ok(until);
        }
        now.checked_add(Duration::from_secs(15)).map_err(CampaignError::Time)
    }

    /// Retain the first actual close interval before cleanup admission can fail or be delayed.
    pub(super) fn freeze_cleanup_deadline(&mut self, now: Moment) -> Result<(), CampaignError> {
        if self.owner.is_some() && self.cleanup_until.is_none() {
            self.cleanup_until = Some(self.cleanup_deadline(now)?);
        }
        Ok(())
    }
}
