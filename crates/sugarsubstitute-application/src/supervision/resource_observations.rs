// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit actual claim, launch, readiness and cleanup facts before policy progress.
use super::{resource_state::ResourcePhase, resources::ResourceCoordinator};
use sugarsubstitute_contracts::lifecycle::*;

impl ResourceCoordinator {
    /// Apply one trusted adapter fact; cancellation/kill requests are not facts.
    ///
    /// # Errors
    /// Rejects foreign/stale generations, invalid claim/process ordering, wrong
    /// endpoints and repeated failures without mutation, returning the owned input.
    pub fn observe(
        &mut self,
        observation: ResourceObservation,
    ) -> Result<ResourceTransition, Box<RejectedObservation<ResourceObservation>>> {
        if let Err(reason) = self.validate_observation(&observation) {
            return Err(Box::new(RejectedObservation { reason, observation }));
        }
        let key = observation.resource();
        let mut transition = ResourceTransition::default();
        match observation {
            ResourceObservation::ClaimAcquired(_) => {
                if let Some(resource) = self.resources.get_mut(&key.id) {
                    resource.claim = true;
                    if !resource.cleanup_requested {
                        resource.phase = ResourcePhase::Claimed;
                    }
                }
            }
            ResourceObservation::OwnedStarted { process, .. } => {
                if let Some(resource) = self.resources.get_mut(&key.id) {
                    resource.process = Some(process);
                    if !resource.cleanup_requested {
                        resource.phase = ResourcePhase::Starting;
                    }
                }
            }
            ResourceObservation::Ready { .. } => {
                let leases = self
                    .resources
                    .get(&key.id)
                    .map(|r| r.leases.values().copied().collect::<Vec<_>>())
                    .unwrap_or_default();
                if let Some(resource) = self.resources.get_mut(&key.id) {
                    resource.phase = ResourcePhase::Ready;
                }
                for lease in leases {
                    self.finish_pending(lease, RequestOutcome::Granted(lease), &mut transition);
                }
                self.ready_replacements(key, &mut transition);
            }
            ResourceObservation::ManagedExited { status, failure, .. } => {
                self.invalidate_granted(key, status, &mut transition);
                self.apply_failure(key, failure, &mut transition);
            }
            ResourceObservation::Failed { failure, .. } => {
                self.apply_failure(key, failure, &mut transition);
            }
            ResourceObservation::Settled(_) => {
                self.settled_outages(key);
                if let Some(resource) = self.resources.get_mut(&key.id) {
                    resource.phase = ResourcePhase::Settled;
                    resource.claim = false;
                    resource.process = None;
                }
                for run in self.runs.values_mut() {
                    if run.resources.get(&key.id) == Some(&key) {
                        run.resources.remove(&key.id);
                    }
                }
            }
        }
        Ok(transition)
    }
    fn apply_failure(
        &mut self,
        key: ResourceKey,
        failure: LifecycleFailure,
        transition: &mut ResourceTransition,
    ) {
        self.failed_replacements(key);
        let leases = self
            .resources
            .get(&key.id)
            .map(|r| r.leases.values().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        if let Some(resource) = self.resources.get_mut(&key.id) {
            resource.failure = Some(failure);
            resource.phase = ResourcePhase::Uncertain;
            resource.leases.clear();
            if let Some(effect) = resource.cleanup() {
                transition.effects.push(effect);
            }
        }
        for lease in leases {
            self.finish_pending(lease, RequestOutcome::Failed, transition);
        }
    }
    fn validate_observation(
        &self,
        observation: &ResourceObservation,
    ) -> Result<(), LifecycleRejection> {
        let key = observation.resource();
        if key.owner != self.owner {
            return Err(LifecycleRejection::ForeignOwner);
        }
        let resource = self
            .resources
            .get(&key.id)
            .filter(|r| r.key == key)
            .ok_or(LifecycleRejection::StaleIdentity)?;
        let managed = matches!(resource.target, BackendTarget::Managed(_));
        let cleanup = matches!(resource.phase, ResourcePhase::Stopping | ResourcePhase::Uncertain);
        let valid = match observation {
            ResourceObservation::ClaimAcquired(_) => {
                managed
                    && !resource.claim
                    && (resource.phase == ResourcePhase::Preparing || cleanup)
            }
            ResourceObservation::OwnedStarted { process, .. } => {
                if process.owner != self.owner {
                    return Err(LifecycleRejection::ForeignOwner);
                }
                if self.resources.values().any(|other| other.process == Some(*process)) {
                    return Err(LifecycleRejection::StaleIdentity);
                }
                managed
                    && resource.claim
                    && resource.process.is_none()
                    && (resource.phase == ResourcePhase::Claimed || cleanup)
            }
            ResourceObservation::Ready { origin, .. } => {
                let phase =
                    if managed { ResourcePhase::Starting } else { ResourcePhase::Preparing };
                if resource.phase != phase {
                    return Err(LifecycleRejection::WrongPhase);
                }
                let matches = match &resource.target {
                    BackendTarget::Managed(config) => config.port() == origin.port(),
                    BackendTarget::Attached(expected) => expected == origin,
                };
                if !matches {
                    return Err(LifecycleRejection::EndpointMismatch);
                }
                true
            }
            ResourceObservation::ManagedExited { process, failure, .. } => {
                managed
                    && resource.process == Some(*process)
                    && matches!(resource.phase, ResourcePhase::Starting | ResourcePhase::Ready)
                    && resource.failure.is_none()
                    && failure.code() == LifecycleFailureCode::UnexpectedExit
            }
            ResourceObservation::Failed { .. } => {
                resource.phase != ResourcePhase::Settled && resource.failure.is_none()
            }
            ResourceObservation::Settled(_) => {
                cleanup && resource.leases.is_empty() && resource.cleanup_requested
            }
        };
        if valid { Ok(()) } else { Err(LifecycleRejection::WrongPhase) }
    }
}
