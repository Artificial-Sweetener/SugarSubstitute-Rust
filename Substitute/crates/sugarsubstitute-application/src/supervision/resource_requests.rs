// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit exact requests and retain bounded duplicate/outcome history.
use super::{resource_state::*, resources::ResourceCoordinator};
use sugarsubstitute_contracts::lifecycle::*;

impl ResourceCoordinator {
    /// Admit one connection-bound request or replay its recorded outcome only.
    ///
    /// # Errors
    /// Rejects stale/foreign identities, changed duplicates and bounded overload
    /// before mutation. Immediate release/reload do not consume pending credits.
    /// Completed history eviction never allows an earlier request ID to run again.
    pub fn request(
        &mut self,
        binding: ClientBinding,
        request: ResourceRequest,
    ) -> Result<ResourceTransition, LifecycleRejection> {
        if let Some(previous) = self.checked_request(binding, &request)? {
            return Ok(ResourceTransition {
                replies: vec![previous],
                effects: vec![],
                invalidations: vec![],
                replacements: vec![],
            });
        }
        let mut transition = ResourceTransition::default();
        let outcome = match &request.command {
            ResourceCommand::Acquire(target) => {
                self.acquire(&request, target, &mut transition, false)?
            }
            ResourceCommand::RetryDead(failed) => {
                self.retry_dead(&request, *failed, &mut transition)?
            }
            ResourceCommand::Release(lease) => {
                self.validate_lease(&request, *lease)?;
                self.release_lease(*lease, &mut transition)?;
                RequestOutcome::Released
            }
            ResourceCommand::Reload(lease) => {
                self.validate_lease(&request, *lease)?;
                if self.resource_phase(lease.resource) != Some(ResourcePhase::Ready) {
                    return Err(LifecycleRejection::WrongPhase);
                }
                RequestOutcome::Reloaded(*lease)
            }
        };
        let reply = ResourceReply { run: request.run, request: request.id, outcome };
        let record = RequestRecord { command: request.command, reply };
        if let Some(run) = self.runs.get_mut(&request.run.instance) {
            run.highwater = request.id.get();
            if matches!(outcome, RequestOutcome::Pending(_)) {
                run.pending.insert(request.id, record);
            } else {
                run.finish(record);
            }
        }
        transition.replies.push(reply);
        Ok(transition)
    }
    /// Inspect exact authenticated replay without changing request or resource state.
    /// The caller skips only fresh source/admission preparation for Ok(true), then
    /// still calls request to obtain the authoritative cached reply with no effects.
    /// # Errors
    /// Preserves request's foreign/stale binding, changed-duplicate and evicted-ID
    /// rejection rules. This result grants no resource, readiness or native proof.
    pub fn request_replay(
        &self,
        binding: ClientBinding,
        request: &ResourceRequest,
    ) -> Result<bool, LifecycleRejection> {
        Ok(self.checked_request(binding, request)?.is_some())
    }

    fn checked_request(
        &self,
        binding: ClientBinding,
        request: &ResourceRequest,
    ) -> Result<Option<ResourceReply>, LifecycleRejection> {
        if binding.run != request.run || binding.connection != request.connection {
            return Err(LifecycleRejection::StaleIdentity);
        }
        self.check_run(request.run, request.connection)?;
        let run = self.runs.get(&request.run.instance).ok_or(LifecycleRejection::StaleIdentity)?;
        if let Some(previous) = run
            .pending
            .get(&request.id)
            .or_else(|| run.history.iter().find(|record| record.reply.request == request.id))
        {
            if previous.command != request.command {
                return Err(LifecycleRejection::ChangedDuplicate);
            }
            return Ok(Some(previous.reply));
        }
        if request.id.get() <= run.highwater {
            return Err(LifecycleRejection::StaleRequest);
        }
        Ok(None)
    }
    /// Release the original authenticated attempt lease without disconnecting its run.
    /// # Errors
    /// Rejects foreign/stale binding or lease before changing the actual resource.
    pub fn release_attempt(
        &mut self,
        binding: ClientBinding,
        lease: LeaseKey,
        transition: &mut ResourceTransition,
    ) -> Result<(), LifecycleRejection> {
        self.check_run(binding.run, binding.connection)?;
        if lease.run != binding.run
            || lease.connection != binding.connection
            || lease.resource.owner != self.owner
            || self.resources.get(&lease.resource.id).is_none_or(|resource| {
                resource.key != lease.resource || resource.leases.get(&lease.id) != Some(&lease)
            })
        {
            return Err(LifecycleRejection::StaleIdentity);
        }
        self.release_lease(lease, transition)
    }
    /// Observe a released attempt while remaining clients retain a ready shared resource.
    /// # Errors
    /// Rejects foreign binding or another resource generation.
    pub fn attempt_released(
        &self,
        binding: ClientBinding,
        lease: LeaseKey,
    ) -> Result<bool, LifecycleRejection> {
        self.check_run(binding.run, binding.connection)?;
        if lease.run != binding.run
            || lease.connection != binding.connection
            || lease.resource.owner != self.owner
        {
            return Err(LifecycleRejection::StaleIdentity);
        }
        let resource = self
            .resources
            .get(&lease.resource.id)
            .filter(|resource| resource.key == lease.resource)
            .ok_or(LifecycleRejection::StaleIdentity)?;
        Ok(resource.phase == ResourcePhase::Ready
            && !resource.leases.contains_key(&lease.id)
            && !resource.leases.is_empty())
    }
    pub(super) fn acquire(
        &mut self,
        request: &ResourceRequest,
        target: &BackendTarget,
        transition: &mut ResourceTransition,
        transfer: bool,
    ) -> Result<RequestOutcome, LifecycleRejection> {
        let existing = self.resources.values().find(|r| r.target == *target);
        if existing.is_some_and(|r| {
            matches!(r.phase, ResourcePhase::Stopping | ResourcePhase::Uncertain)
                || r.failure.is_some()
                || (!transfer
                    && r.phase == ResourcePhase::Settled
                    && self.retains_outage_resource(r.key.id))
        }) {
            return Err(LifecycleRejection::WrongPhase);
        }
        let ready = existing.is_some_and(|r| r.phase == ResourcePhase::Ready);
        let run = self.runs.get(&request.run.instance).ok_or(LifecycleRejection::StaleIdentity)?;
        if self.retained_lease_associations().saturating_sub(usize::from(transfer)) >= MAX_LEASES
            || (!ready
                && (run.pending.len() >= MAX_RUN_REQUESTS
                    || self.snapshot().outstanding_requests >= MAX_REQUESTS))
        {
            return Err(LifecycleRejection::Overloaded);
        }
        let lease_value =
            self.next_lease.checked_add(1).ok_or(LifecycleRejection::IdentityExhausted)?;
        let lease_id =
            LeaseId::new(lease_value).map_err(|_| LifecycleRejection::IdentityExhausted)?;
        let key = if let Some(record) = existing {
            if record.phase == ResourcePhase::Settled {
                ResourceKey {
                    generation: ResourceGeneration::new(
                        record
                            .key
                            .generation
                            .get()
                            .checked_add(1)
                            .ok_or(LifecycleRejection::IdentityExhausted)?,
                    )
                    .map_err(|_| LifecycleRejection::IdentityExhausted)?,
                    ..record.key
                }
            } else {
                record.key
            }
        } else {
            if self.resources.len() >= MAX_RESOURCES
                && !self.resources.values().any(|r| {
                    r.phase == ResourcePhase::Settled
                        && r.failure.is_none()
                        && !self.retains_outage_resource(r.key.id)
                })
            {
                return Err(LifecycleRejection::Overloaded);
            }
            ResourceKey {
                owner: self.owner,
                id: ResourceId::new(
                    self.next_resource
                        .checked_add(1)
                        .ok_or(LifecycleRejection::IdentityExhausted)?,
                )
                .map_err(|_| LifecycleRejection::IdentityExhausted)?,
                generation: ResourceGeneration::new(1)
                    .map_err(|_| LifecycleRejection::IdentityExhausted)?,
            }
        };
        let fresh = self.resources.get(&key.id).is_none_or(|r| r.key != key);
        if fresh {
            if !self.resources.contains_key(&key.id) && self.resources.len() >= MAX_RESOURCES {
                let retired = self
                    .resources
                    .iter()
                    .find(|(_, r)| {
                        r.phase == ResourcePhase::Settled
                            && r.failure.is_none()
                            && !self.retains_outage_resource(r.key.id)
                    })
                    .map(|(id, _)| *id);
                if let Some(retired) = retired {
                    self.resources.remove(&retired);
                }
            }
            self.next_resource = self.next_resource.max(key.id.get());
            self.resources.insert(
                key.id,
                ResourceRecord {
                    key,
                    target: target.clone(),
                    phase: ResourcePhase::Preparing,
                    claim: false,
                    process: None,
                    leases: Default::default(),
                    cleanup_requested: false,
                    failure: None,
                },
            );
            transition
                .effects
                .push(ResourceEffect::Prepare { resource: key, target: target.clone() });
        }
        let lease = LeaseKey {
            run: request.run,
            resource: key,
            connection: request.connection,
            id: lease_id,
        };
        if let Some(resource) = self.resources.get_mut(&key.id) {
            resource.leases.insert(lease.id, lease);
        }
        if let Some(run) = self.runs.get_mut(&request.run.instance) {
            run.resources.insert(key.id, key);
        }
        self.next_lease = lease_value;
        Ok(if ready { RequestOutcome::Granted(lease) } else { RequestOutcome::Pending(lease) })
    }
    fn validate_lease(
        &self,
        request: &ResourceRequest,
        lease: LeaseKey,
    ) -> Result<(), LifecycleRejection> {
        if lease.run != request.run
            || lease.connection != request.connection
            || lease.resource.owner != self.owner
            || self
                .resources
                .get(&lease.resource.id)
                .is_none_or(|r| r.key != lease.resource || r.leases.get(&lease.id) != Some(&lease))
        {
            return Err(LifecycleRejection::StaleIdentity);
        }
        Ok(())
    }
    pub(super) fn release_lease(
        &mut self,
        lease: LeaseKey,
        transition: &mut ResourceTransition,
    ) -> Result<(), LifecycleRejection> {
        self.outages.retain(|_, outage| outage.replacement != Some(lease));
        let resource =
            self.resources.get_mut(&lease.resource.id).ok_or(LifecycleRejection::StaleIdentity)?;
        resource.leases.remove(&lease.id);
        if resource.leases.is_empty()
            && let Some(effect) = resource.cleanup()
        {
            transition.effects.push(effect);
        }
        self.finish_pending(lease, RequestOutcome::Cancelled, transition);
        Ok(())
    }
    pub(super) fn finish_pending(
        &mut self,
        lease: LeaseKey,
        outcome: RequestOutcome,
        transition: &mut ResourceTransition,
    ) {
        if let Some(run) = self.runs.get_mut(&lease.run.instance) {
            let request = run
                .pending
                .iter()
                .find(|(_, r)| r.reply.outcome == RequestOutcome::Pending(lease))
                .map(|(id, _)| *id);
            if let Some(request) = request
                && let Some(mut record) = run.pending.remove(&request)
            {
                record.reply.outcome = outcome;
                transition.replies.push(record.reply);
                run.finish(record);
            }
        }
    }
}
