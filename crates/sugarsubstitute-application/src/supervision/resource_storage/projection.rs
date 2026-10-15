// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project additional layouts with borrowed iterator counters, never a sizing allocation.
use super::layout::{Bytes, add, multiply, tree, tree_growth};
use super::{ResourceStorageInput, command_bytes, target_bytes};
use crate::supervision::{
    InvalidatedLeaseState, resource_state::*, resources::ResourceCoordinator,
};
use sugarsubstitute_contracts::lifecycle::*;

impl ResourceCoordinator {
    pub(super) fn projected_layouts(&self, input: ResourceStorageInput<'_>) -> Bytes {
        match input {
            ResourceStorageInput::Current => Ok(0),
            ResourceStorageInput::Enrollment(enrollment) => {
                if self
                    .runs
                    .get(&enrollment.run.instance)
                    .is_some_and(|run| run.enrollment == *enrollment && !run.settled)
                {
                    return Ok(0);
                }
                let mut bytes = enrollment.profile.allocated_bytes();
                if !self.runs.contains_key(&enrollment.run.instance) {
                    bytes = add(bytes, tree_growth::<InstanceId, RunRecord>(1)?)?;
                }
                bytes = add(bytes, tree::<RequestId, RequestRecord>(0)?)?;
                add(bytes, tree::<ResourceId, ResourceKey>(0)?)
            }
            ResourceStorageInput::Request(request) => self.request_layouts(request),
            ResourceStorageInput::Observation(_) | ResourceStorageInput::Disconnect(_) => Ok(0),
        }
    }

    fn request_layouts(&self, request: &ResourceRequest) -> Bytes {
        let Some(run) = self.retaining_request(request) else {
            return Ok(0);
        };
        let mut bytes = 0;
        bytes = add(bytes, command_bytes(&request.command)?)?;
        match &request.command {
            ResourceCommand::Acquire(target) => {
                // Incoming command is moved; fresh record and Prepare each clone
                // the validated target. Pinned Global clones allocate no more
                // than the source's actual path/vector/string capacities.
                bytes = add(bytes, multiply(2, target_bytes(target)?)?)?;
                bytes = add(bytes, tree_growth::<RequestId, RequestRecord>(1)?)?;
                bytes = add(bytes, tree_growth::<LeaseId, LeaseKey>(1)?)?;
                let existing = self.resources.values().find(|record| record.target == *target);
                if existing.is_none() {
                    bytes = add(bytes, tree_growth::<ResourceId, ResourceRecord>(1)?)?;
                    bytes = add(bytes, tree::<LeaseId, LeaseKey>(0)?)?;
                }
                if existing.is_none_or(|record| !run.resources.contains_key(&record.key.id)) {
                    bytes = add(bytes, tree_growth::<ResourceId, ResourceKey>(1)?)?;
                }
                Ok(bytes)
            }
            ResourceCommand::RetryDead(failed) => {
                let binding = ClientBinding { run: request.run, connection: request.connection };
                if self.invalidated_lease(binding, *failed) != Ok(InvalidatedLeaseState::RetryReady)
                {
                    return Ok(0);
                }
                let cohort = self
                    .outages
                    .values()
                    .filter(|outage| outage.notice.lease.resource.id == failed.resource.id)
                    .count();
                if let Some(resource) = self.resources.get(&failed.resource.id) {
                    // Local retry target, new record target and returned Prepare.
                    bytes = add(bytes, multiply(3, target_bytes(&resource.target)?)?)?;
                }
                bytes = add(bytes, tree_growth::<RequestId, RequestRecord>(1)?)?;
                bytes = add(bytes, tree_growth::<LeaseId, LeaseKey>(cohort)?)?;
                for member in self.runs.values() {
                    if !member.resources.contains_key(&failed.resource.id)
                        && self.outages.values().any(|outage| {
                            outage.notice.lease.resource.id == failed.resource.id
                                && outage.notice.lease.run == member.enrollment.run
                        })
                    {
                        bytes = add(bytes, tree_growth::<ResourceId, ResourceKey>(1)?)?;
                    }
                }
                Ok(bytes)
            }
            ResourceCommand::Release(_) | ResourceCommand::Reload(_) => Ok(bytes),
        }
    }
}
