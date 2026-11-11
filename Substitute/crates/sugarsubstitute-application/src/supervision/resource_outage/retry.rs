// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer the same association credit only on an explicit authenticated retry.
use super::InvalidatedLeaseState;
use crate::supervision::resources::ResourceCoordinator;
use sugarsubstitute_contracts::lifecycle::*;

impl ResourceCoordinator {
    pub(in crate::supervision) fn retry_dead(
        &mut self,
        request: &ResourceRequest,
        failed: LeaseKey,
        transition: &mut ResourceTransition,
    ) -> Result<RequestOutcome, LifecycleRejection> {
        let binding = ClientBinding { run: request.run, connection: request.connection };
        if self.checked_outage(binding, failed)?.state() != InvalidatedLeaseState::RetryReady {
            return Err(LifecycleRejection::WrongPhase);
        }
        let mut cohort: Vec<_> = self
            .outages
            .values()
            .filter(|outage| outage.notice.lease.resource.id == failed.resource.id)
            .map(|outage| (outage.notice.lease, outage.state()))
            .collect();
        if cohort.iter().any(|(_, state)| *state != InvalidatedLeaseState::RetryReady) {
            return Err(LifecycleRejection::WrongPhase);
        }
        // Put the requesting client's exact association first; all others retain
        // an interest before the single Prepare effect can leave this owner.
        cohort.sort_by_key(|(lease, _)| lease.id != failed.id);
        let planned: Vec<_> = cohort
            .iter()
            .enumerate()
            .map(|(index, (previous, _))| {
                let value = self
                    .next_lease
                    .checked_add(index as u64 + 1)
                    .ok_or(LifecycleRejection::IdentityExhausted)?;
                Ok((
                    *previous,
                    LeaseId::new(value).map_err(|_| LifecycleRejection::IdentityExhausted)?,
                ))
            })
            .collect::<Result<_, LifecycleRejection>>()?;
        let target = self
            .resources
            .get(&failed.resource.id)
            .map(|resource| &resource.target)
            .filter(|target| matches!(target, BackendTarget::Managed(_)))
            .ok_or(LifecycleRejection::StaleIdentity)?
            .clone();
        let outcome = self.acquire(request, &target, transition, true)?;
        if let RequestOutcome::Pending(first) | RequestOutcome::Granted(first) = outcome {
            for (previous, id) in planned {
                let replacement = LeaseKey {
                    run: previous.run,
                    connection: previous.connection,
                    resource: first.resource,
                    id,
                };
                if let Some(resource) = self.resources.get_mut(&first.resource.id) {
                    resource.leases.insert(id, replacement);
                }
                if let Some(run) = self.runs.get_mut(&previous.run.instance) {
                    run.resources.insert(first.resource.id, first.resource);
                }
                if let Some(outage) = self.outages.get_mut(&previous.id) {
                    outage.replacement = Some(replacement);
                    outage.retry_failed = false;
                    outage.retry_settled = false;
                    outage.retry_cause_transferred = false;
                }
                self.next_lease = id.get();
            }
        }
        Ok(outcome)
    }
}
