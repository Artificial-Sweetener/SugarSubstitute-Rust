// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Size actual policy custody and a projected operation before its first allocation.
use super::{resource_outage::RetainedOutage, resource_state::*, resources::ResourceCoordinator};
use sugarsubstitute_contracts::lifecycle::*;

mod layout;
mod peaks;
mod projection;
use layout::{Bytes, add, multiply, tree};

/// Borrow operation data without admitting it, cloning inputs or doing native I/O.
/// Current also covers register/settle/adopt/failure-transfer, which do not grow
/// policy storage. Caller-owned input/source allocations remain charged elsewhere
/// until an actual transfer into this policy; this value grants no native proof.
#[derive(Clone, Copy)]
pub enum ResourceStorageInput<'a> {
    /// Size the actual retained policy and its bounded transition/scratch reserve.
    Current,
    /// Preflight an enrollment's actual profile capacity and possible new run.
    Enrollment(&'a RunEnrollment),
    /// Preflight exact request retention, target copies, history and cohort growth.
    Request(&'a ResourceRequest),
    /// Preflight trusted observation retention and pending-outcome history growth.
    Observation(&'a ResourceObservation),
    /// Preflight pending cancellations for the exact disconnected client.
    Disconnect(ClientBinding),
}

/// Conservative requested Rust layouts; not a memory lease or native admission.
/// Opaque original error/source owners, allocator/OS internals and compiled call
/// stacks are separate caller-funded roles. No arbitrary Error is introspected.
/// One new returned transition is covered; prior undelivered transitions and any
/// extra Host effect/target clones require their own caller-owned capacity charge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceStorageRequirement {
    fixed: u64,
    retained: u64,
    projected: u64,
    transient: u64,
    returned: u64,
    allocated: u64,
    total: u64,
}

impl ResourceStorageRequirement {
    /// Inline coordinator bytes, already included when its actual containing Host is charged.
    pub const fn fixed_bytes(self) -> u64 {
        self.fixed
    }
    /// Existing heap layouts and actual retained dynamic capacities.
    pub const fn retained_bytes(self) -> u64 {
        self.retained
    }
    /// Conservative additional storage for the borrowed operation, before mutation.
    pub const fn projected_bytes(self) -> u64 {
        self.projected
    }
    /// Bounded returned transitions, temporary vectors, sort buffers and set nodes.
    pub const fn transient_bytes(self) -> u64 {
        self.transient
    }
    /// Deep allocation upper bound for copying ONE new returned transition.
    /// This excludes App-only sort/set/cohort scratch, projected command/record
    /// retention and mandatory history/outage destinations. Extra caller copies
    /// use this bound, while an already-held result is counted by actual capacity.
    pub const fn returned_transition_bytes(self) -> u64 {
        self.returned
    }
    /// Retained + projected + transient; add this to an already-funded containing owner.
    pub const fn allocation_bytes(self) -> u64 {
        self.allocated
    }
    /// Fixed + allocation bytes, when funding the coordinator independently.
    pub const fn total_bytes(self) -> u64 {
        self.total
    }
    /// Check an actual caller partition without creating or transferring a claim.
    /// # Errors
    /// Returns Overloaded before mutation when the existing allocation partition
    /// cannot cover this requirement. Native/source owners remain caller-held.
    pub const fn check_allocation_partition(
        self,
        available: u64,
    ) -> Result<(), LifecycleRejection> {
        if self.allocated <= available { Ok(()) } else { Err(LifecycleRejection::Overloaded) }
    }
}

impl ResourceCoordinator {
    /// Preflight actual retained state plus conservative operation growth without allocating.
    ///
    /// The receiving adapter compares allocation_bytes with its actual remaining
    /// policy partition BEFORE calling the retaining operation or allocating its
    /// result. Holding this data is not reservation or admission: repeat after any
    /// policy mutation. Logical caps do not guarantee maximum-sized inputs fit.
    ///
    /// Pinned Rust1.93.1 node/Global layout and collection growth assumptions are
    /// documented in the private layout owner. Node/control/alignment costs and
    /// actual history spare capacity are included, independently of live credits.
    ///
    /// # Errors
    /// Returns Overloaded on checked arithmetic failure. Caller keeps all original
    /// inputs/native causes; this read-only method changes no policy outcome.
    pub fn storage_preflight(
        &self,
        input: ResourceStorageInput<'_>,
    ) -> Result<ResourceStorageRequirement, LifecycleRejection> {
        let retained = self.retained_layouts()?;
        let projected = self.projected_layouts(input)?;
        let peaks = self.allocation_peaks(input)?;
        let transient = peaks.total()?;
        let allocated = add(add(retained, projected)?, transient)?;
        let total = add(size_of::<Self>(), allocated)?;
        let bytes = |value| u64::try_from(value).map_err(|_| LifecycleRejection::Overloaded);
        Ok(ResourceStorageRequirement {
            fixed: bytes(size_of::<Self>())?,
            retained: bytes(retained)?,
            projected: bytes(projected)?,
            transient: bytes(transient)?,
            returned: bytes(peaks.returned)?,
            allocated: bytes(allocated)?,
            total: bytes(total)?,
        })
    }

    fn retained_layouts(&self) -> Bytes {
        let mut bytes = add(
            tree::<InstanceId, RunRecord>(self.runs.len())?,
            tree::<ResourceId, ResourceRecord>(self.resources.len())?,
        )?;
        bytes = add(bytes, tree::<LeaseId, RetainedOutage>(self.outages.len())?)?;
        for run in self.runs.values() {
            bytes = add(bytes, run.enrollment.profile.allocated_bytes())?;
            bytes = add(bytes, tree::<RequestId, RequestRecord>(run.pending.len())?)?;
            bytes = add(bytes, tree::<ResourceId, ResourceKey>(run.resources.len())?)?;
            bytes = add(bytes, multiply(run.history.capacity(), size_of::<RequestRecord>())?)?;
            for record in run.pending.values().chain(run.history.iter()) {
                bytes = add(bytes, command_bytes(&record.command)?)?;
            }
        }
        for record in self.resources.values() {
            bytes = add(bytes, target_bytes(&record.target)?)?;
            bytes = add(bytes, tree::<LeaseId, LeaseKey>(record.leases.len())?)?;
        }
        Ok(bytes)
    }

    fn retaining_request(&self, request: &ResourceRequest) -> Option<&RunRecord> {
        let binding = ClientBinding { run: request.run, connection: request.connection };
        if self.request_replay(binding, request) != Ok(false) {
            return None;
        }
        self.runs.get(&request.run.instance)
    }
}

fn target_bytes(target: &BackendTarget) -> Bytes {
    match target {
        BackendTarget::Managed(config) => {
            config.allocated_bytes().ok_or(LifecycleRejection::Overloaded)
        }
        BackendTarget::Attached(_) => Ok(0),
    }
}

fn command_bytes(command: &ResourceCommand) -> Bytes {
    match command {
        ResourceCommand::Acquire(target) => target_bytes(target),
        ResourceCommand::RetryDead(_)
        | ResourceCommand::Release(_)
        | ResourceCommand::Reload(_) => Ok(0),
    }
}
