// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Separate finite preparation/probes from an explicitly admitted live managed wait.
use super::*;
use std::time::Duration;

impl ReadinessCampaign {
    /// Admit elapsed-time-independent readiness after the host's actual owned child-start observation.
    ///
    /// The host must keep that matching child's exit watch and cancellation owner.
    /// This transition observes no process and grants no launch/listener authority.
    /// Initial preparation must still be current; no actual probe may precede it.
    /// Replaying an admitted live transition never changes the original budget.
    ///
    /// # Errors
    /// Rejects attached targets, terminal state, pending probe custody, regressing
    /// clocks and first admission at or after the finite preparation deadline.
    pub fn admit_managed_live(&mut self, now: Moment) -> Result<(), CampaignError> {
        self.validate_now(now)?;
        if !matches!(self.target, BackendTarget::Managed(_)) {
            return Err(CampaignError::Rejected(CampaignRejection::WrongTarget));
        }
        if self.terminal {
            return Err(CampaignError::Rejected(CampaignRejection::WrongPhase));
        }
        if self.managed_live {
            return Ok(());
        }
        if self.ready || self.owner.is_some() || self.attempt.is_some() || self.cleanup.is_some() {
            return Err(CampaignError::Rejected(CampaignRejection::WrongPhase));
        }
        self.budget.probe_until(now).map_err(CampaignError::Budget)?;
        self.managed_live = true;
        self.last_now = now;
        Ok(())
    }

    /// Return the original finite cutoff, or no aggregate cutoff after owned managed-live admission.
    pub const fn campaign_until(&self) -> Option<Moment> {
        if self.managed_live { None } else { Some(self.budget.until()) }
    }

    /// Bound each actual probe by two seconds and any still-applicable finite cutoff.
    ///
    /// # Errors
    /// Preserves original clock/expiry causes; live managed probes still reject
    /// regression and checked addition overflow rather than renewing an old job.
    pub fn probe_until(&self, now: Moment) -> Result<Moment, CampaignError> {
        self.validate_now(now)?;
        if self.managed_live {
            self.budget.managed_probe_until(now)
        } else {
            self.budget.probe_until(now)
        }
        .map_err(CampaignError::Budget)
    }

    /// Return the first retained cleanup cutoff for the current probe's actual outputs.
    ///
    /// Final cancellation uses its original shutdown settle deadline. A live
    /// managed retry freezes at most fifteen seconds at terminal-result adoption;
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
