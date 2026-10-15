// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fund actual interests' mandatory outcomes separately from returned-result copies.
use super::layout::{Bytes, add, history_growth, multiply, tree, tree_growth, vector};
use super::{ResourceStorageInput, ResourceStorageRequirement, target_bytes};
use crate::supervision::{
    InvalidatedLeaseState, resource_state::*, resources::ResourceCoordinator,
};
use sugarsubstitute_contracts::lifecycle::*;

/// Every component is a requested-layout bound, never allocation authority.
pub(super) struct AllocationPeaks {
    pub(super) returned: usize,
    scratch: usize,
    liabilities: usize,
}
impl AllocationPeaks {
    pub(super) fn total(&self) -> Bytes {
        add(add(self.returned, self.scratch)?, self.liabilities)
    }
}
struct FutureCounts {
    associations: usize,
    runs: usize,
    resources: usize,
    request: bool,
}
impl ResourceCoordinator {
    pub(super) fn allocation_peaks(
        &self,
        input: ResourceStorageInput<'_>,
    ) -> Result<AllocationPeaks, LifecycleRejection> {
        let counts = self.future_counts(input)?;
        let returned = self.returned_layout(input, &counts)?;
        let scratch = Self::scratch_layout(&counts)?;
        let liabilities = self.mandatory_growth(input, counts.associations)?;
        Ok(AllocationPeaks { returned, scratch, liabilities })
    }

    fn future_counts(
        &self,
        input: ResourceStorageInput<'_>,
    ) -> Result<FutureCounts, LifecycleRejection> {
        let request = match input {
            ResourceStorageInput::Request(request) if self.retaining_request(request).is_some() => {
                Some(request)
            }
            _ => None,
        };
        let acquire = request.and_then(|request| match &request.command {
            ResourceCommand::Acquire(target) => Some(target),
            _ => None,
        });
        let associations = add(self.retained_lease_associations(), usize::from(acquire.is_some()))?;
        if associations > MAX_LEASES {
            return Err(LifecycleRejection::Overloaded);
        }
        let new_run = matches!(input, ResourceStorageInput::Enrollment(enrollment)
            if !self.runs.contains_key(&enrollment.run.instance));
        let runs = add(self.runs.len(), usize::from(new_run))?.min(MAX_RUNS);
        let new_resource = acquire
            .is_some_and(|target| !self.resources.values().any(|record| record.target == *target));
        let resources = add(self.resources.len(), usize::from(new_resource))?.min(MAX_RESOURCES);
        Ok(FutureCounts {
            associations,
            runs,
            resources,
            request: matches!(input, ResourceStorageInput::Request(_)),
        })
    }

    /// One mandatory observation can finish every pending lease, emit every
    /// replacement or invalidate the same actual interest set. Disconnect can
    /// cancel those requests and clean up at most the actual resource records.
    /// Reserve these destinations at admission, including one new Acquire.
    fn returned_layout(&self, input: ResourceStorageInput<'_>, counts: &FutureCounts) -> Bytes {
        // Release can finish its old pending Acquire and report the new request.
        // Other requests return one reply; replay does not create another effect.
        let replies = counts.associations.max(if counts.request { 2 } else { 0 });
        let prepare = self.returned_prepare(input);
        let effects = counts.resources.max(usize::from(prepare.is_some()));
        let mut bytes = vector::<ResourceReply>(replies)?;
        bytes = add(bytes, vector::<ResourceEffect>(effects)?)?;
        bytes = add(bytes, vector::<GrantedResourceInvalidation>(counts.associations)?)?;
        bytes = add(bytes, vector::<GrantedResourceReplacement>(counts.associations)?)?;
        if let Some(target) = prepare {
            bytes = add(bytes, target_bytes(target)?)?;
        }
        Ok(bytes)
    }

    /// The result copy contains at most one Prepare target. App-internal moved
    /// commands, record clones, cohort plans and sorting do not belong to it.
    fn returned_prepare<'a>(
        &'a self,
        input: ResourceStorageInput<'a>,
    ) -> Option<&'a BackendTarget> {
        let ResourceStorageInput::Request(request) = input else { return None };
        self.retaining_request(request)?;
        match &request.command {
            ResourceCommand::Acquire(target) => Some(target),
            ResourceCommand::RetryDead(failed) => {
                self.resources.get(&failed.resource.id).map(|record| &record.target)
            }
            ResourceCommand::Release(_) | ResourceCommand::Reload(_) => None,
        }
    }

    /// All possible source-only scratch for the current actual interest set is
    /// reserved even before the corresponding native outcome arrives. Old and
    /// successor map entries count separately in retained_layouts; one pair
    /// consumes one logical association and cannot grow this bound by replay.
    fn scratch_layout(counts: &FutureCounts) -> Bytes {
        let n = counts.associations;
        let mut bytes = size_of::<ResourceTransition>();
        bytes = add(bytes, size_of::<std::collections::BTreeSet<RunKey>>())?;
        bytes = add(bytes, multiply(2, size_of::<Vec<LeaseKey>>())?)?;
        bytes = add(bytes, multiply(2, size_of::<Vec<RunKey>>())?)?;
        bytes = add(bytes, multiply(2, size_of::<Vec<(LeaseKey, InvalidatedLeaseState)>>())?)?;
        bytes = add(bytes, size_of::<Vec<(LeaseKey, LeaseId)>>())?;
        bytes = add(bytes, size_of::<ResourceStorageRequirement>())?;
        bytes = add(bytes, multiply(2, vector::<LeaseKey>(n)?)?)?;
        // `BTreeSet::from_iter` first collects up to n duplicate RunKeys.
        bytes = add(bytes, multiply(2, vector::<RunKey>(n)?)?)?;
        if n != 0 {
            bytes = add(bytes, tree::<RunKey, ()>(n.min(counts.runs))?)?;
        }
        bytes = add(bytes, multiply(2, vector::<(LeaseKey, InvalidatedLeaseState)>(n)?)?)?;
        bytes = add(bytes, vector::<(LeaseKey, LeaseId)>(n)?)?;
        add(bytes, size_of::<RejectedObservation<ResourceObservation>>())
    }

    /// These are mandatory destination liabilities, not Host result copies.
    /// History receives every already-pending outcome plus this fresh request;
    /// every accepted interest can require its own retained outage slot.
    fn mandatory_growth(&self, input: ResourceStorageInput<'_>, associations: usize) -> Bytes {
        let additional_outages =
            associations.checked_sub(self.outages.len()).ok_or(LifecycleRejection::Overloaded)?;
        let mut bytes = tree_growth::<LeaseId, super::super::resource_outage::RetainedOutage>(
            additional_outages,
        )?;
        let retaining = match input {
            ResourceStorageInput::Request(request) if self.retaining_request(request).is_some() => {
                Some(request)
            }
            _ => None,
        };
        for run in self.runs.values() {
            let own_request =
                usize::from(retaining.is_some_and(|request| request.run == run.enrollment.run));
            let pushes = add(run.pending.len(), own_request)?;
            bytes = add(
                bytes,
                history_growth::<RequestRecord>(run.history.capacity(), run.history.len(), pushes)?,
            )?;
        }
        Ok(bytes)
    }
}
