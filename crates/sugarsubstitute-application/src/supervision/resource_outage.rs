// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain failed granted associations until explicit client adoption or disconnect.
use super::{resource_state::ResourcePhase, resources::ResourceCoordinator};
use sugarsubstitute_contracts::lifecycle::*;

mod observation;
mod retry;

/// Inspect logical retry eligibility without granting native readiness or cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidatedLeaseState {
    /// Original native cleanup or explicit first-cause ownership transfer is pending.
    Retiring,
    /// Actual cleanup and cause transfer permit a new user-authorized request.
    RetryReady,
    /// A successor was admitted; readiness, adoption, or its failed cleanup is pending.
    Retrying,
}

pub(super) struct RetainedOutage {
    pub notice: GrantedResourceInvalidation,
    pub affected_clients: u16,
    pub settled: bool,
    pub cause_transferred: bool,
    pub replacement: Option<LeaseKey>,
    pub retry_failed: bool,
    pub retry_settled: bool,
    pub retry_cause_transferred: bool,
}

impl RetainedOutage {
    fn state(&self) -> InvalidatedLeaseState {
        if self.replacement.is_some() {
            if self.retry_failed && self.retry_settled && self.retry_cause_transferred {
                InvalidatedLeaseState::RetryReady
            } else {
                InvalidatedLeaseState::Retrying
            }
        } else if self.settled && self.cause_transferred {
            InvalidatedLeaseState::RetryReady
        } else {
            InvalidatedLeaseState::Retiring
        }
    }
}

impl ResourceCoordinator {
    /// Inspect one exact authenticated failed association, including internal retry.
    ///
    /// # Errors
    /// Rejects foreign, disconnected, or stale bindings and another invalidation.
    /// This query neither transfers an error nor infers native process settlement.
    pub fn invalidated_lease(
        &self,
        binding: ClientBinding,
        failed: LeaseKey,
    ) -> Result<InvalidatedLeaseState, LifecycleRejection> {
        Ok(self.checked_outage(binding, failed)?.state())
    }

    /// Count distinct runs in this invalidation's original granted cohort, at most 64.
    ///
    /// # Errors
    /// Rejects the same stale or unauthenticated associations as `invalidated_lease`.
    pub fn affected_clients(
        &self,
        binding: ClientBinding,
        failed: LeaseKey,
    ) -> Result<u16, LifecycleRejection> {
        Ok(self.checked_outage(binding, failed)?.affected_clients)
    }

    /// Inspect the exact fresh successor retained for this authenticated cohort member.
    ///
    /// # Errors
    /// Rejects stale bindings or another failed association. A returned lease is
    /// retained interest only; actual readiness and native adoption remain separate.
    pub fn replacement_lease(
        &self,
        binding: ClientBinding,
        failed: LeaseKey,
    ) -> Result<Option<LeaseKey>, LifecycleRejection> {
        Ok(self.checked_outage(binding, failed)?.replacement)
    }

    /// Count association credits, including a failed lease and its one successor.
    ///
    /// The pair shares one of the original 256 credits. An admitted successor that
    /// fails retains that credit through its cleanup and first-cause transfer.
    pub fn retained_lease_associations(&self) -> usize {
        let overlap = self
            .outages
            .values()
            .filter_map(|outage| outage.replacement)
            .filter(|lease| self.live_lease(*lease))
            .count();
        self.resources.values().map(|record| record.leases.len()).sum::<usize>()
            + self.outages.len()
            - overlap
    }

    /// Adopt the actual granted replacement after the trusted caller accepts it.
    ///
    /// # Errors
    /// Requires this exact failed/replacement pair, the same authenticated client,
    /// and an actually observed ready generation retaining that live lease. This
    /// logical acknowledgement cannot supply native readiness or settle a process.
    pub fn adopt_replacement(
        &mut self,
        binding: ClientBinding,
        failed: LeaseKey,
        replacement: LeaseKey,
    ) -> Result<(), LifecycleRejection> {
        let outage = self.checked_outage(binding, failed)?;
        if outage.replacement != Some(replacement)
            || replacement.run != binding.run
            || replacement.connection != binding.connection
            || self.resource_phase(replacement.resource) != Some(ResourcePhase::Ready)
            || !self.live_lease(replacement)
        {
            return Err(LifecycleRejection::WrongPhase);
        }
        self.outages.remove(&failed.id);
        Ok(())
    }

    fn checked_outage(
        &self,
        binding: ClientBinding,
        failed: LeaseKey,
    ) -> Result<&RetainedOutage, LifecycleRejection> {
        self.check_run(binding.run, binding.connection)?;
        if failed.run != binding.run || failed.connection != binding.connection {
            return Err(LifecycleRejection::StaleIdentity);
        }
        self.outages
            .get(&failed.id)
            .filter(|outage| outage.notice.lease == failed)
            .ok_or(LifecycleRejection::StaleIdentity)
    }

    fn live_lease(&self, lease: LeaseKey) -> bool {
        self.resources.get(&lease.resource.id).is_some_and(|resource| {
            resource.key == lease.resource && resource.leases.get(&lease.id) == Some(&lease)
        })
    }

    /// Keep an exact original or successor generation while any outage refers to it.
    /// This read-only policy fact grants no process, readiness or retirement proof.
    pub fn retains_outage(&self, resource: ResourceKey) -> bool {
        self.outages.values().any(|outage| {
            outage.notice.lease.resource == resource
                || outage.replacement.is_some_and(|lease| lease.resource == resource)
        })
    }

    /// Prevent unrelated resource-ID reuse across actual retained outage lineage.
    /// Native source matching and fully settled same-target retry remain separate
    /// caller checks; an ID match cannot grant another generation's authority.
    pub fn retains_outage_resource(&self, id: ResourceId) -> bool {
        self.outages.values().any(|outage| {
            outage.notice.lease.resource.id == id
                || outage.replacement.is_some_and(|lease| lease.resource.id == id)
        })
    }
}
