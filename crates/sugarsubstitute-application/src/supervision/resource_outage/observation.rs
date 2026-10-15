// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Record only authoritative managed root death; retain native settlement separately.
use super::RetainedOutage;
use crate::supervision::{resource_state::ResourcePhase, resources::ResourceCoordinator};
use std::collections::BTreeSet;
use sugarsubstitute_contracts::lifecycle::*;

impl ResourceCoordinator {
    pub(in crate::supervision) fn invalidate_granted(
        &mut self,
        resource: ResourceKey,
        status: u32,
        transition: &mut ResourceTransition,
    ) {
        if self.resource_phase(resource) != Some(ResourcePhase::Ready) {
            return;
        }
        let granted: Vec<_> = self
            .resources
            .get(&resource.id)
            .into_iter()
            .flat_map(|record| record.leases.values())
            .filter(|lease| {
                self.runs.get(&lease.run.instance).is_some_and(|run| {
                    !run.pending
                        .values()
                        .any(|pending| pending.reply.outcome == RequestOutcome::Pending(**lease))
                })
            })
            .copied()
            .collect();
        let affected_clients = granted.iter().map(|lease| lease.run).collect::<BTreeSet<_>>().len();
        // MAX_RUNS bounds this lossless conversion independently of the lease count.
        let affected_clients = affected_clients as u16;
        for lease in granted {
            let replacing = self
                .outages
                .iter()
                .find(|(_, outage)| outage.replacement == Some(lease))
                .map(|(id, _)| *id);
            if let Some(previous) = replacing {
                self.outages.remove(&previous);
            }
            let notice = GrantedResourceInvalidation { lease, status };
            self.outages.insert(
                lease.id,
                RetainedOutage {
                    notice,
                    affected_clients,
                    settled: false,
                    cause_transferred: false,
                    replacement: None,
                    retry_failed: false,
                    retry_settled: false,
                    retry_cause_transferred: false,
                },
            );
            transition.invalidations.push(notice);
        }
    }

    pub(in crate::supervision) fn ready_replacements(
        &self,
        resource: ResourceKey,
        transition: &mut ResourceTransition,
    ) {
        for outage in self.outages.values() {
            if let Some(current) = outage.replacement
                && current.resource == resource
                && self.live_lease(current)
            {
                transition
                    .replacements
                    .push(GrantedResourceReplacement { previous: outage.notice.lease, current });
            }
        }
    }

    pub(in crate::supervision) fn failed_replacements(&mut self, resource: ResourceKey) {
        for outage in self.outages.values_mut() {
            if outage.replacement.is_some_and(|lease| lease.resource == resource) {
                outage.retry_failed = true;
            }
        }
    }

    pub(in crate::supervision) fn settled_outages(&mut self, resource: ResourceKey) {
        for outage in self.outages.values_mut() {
            if outage.notice.lease.resource == resource {
                outage.settled = true;
            }
            if outage.replacement.is_some_and(|lease| lease.resource == resource) {
                outage.retry_settled = true;
            }
        }
    }

    pub(in crate::supervision) fn transferred_outage_cause(&mut self, resource: ResourceKey) {
        for outage in self.outages.values_mut() {
            if outage.notice.lease.resource == resource {
                outage.cause_transferred = true;
            }
            if outage.replacement.is_some_and(|lease| lease.resource == resource) {
                outage.retry_cause_transferred = true;
            }
        }
    }
}
