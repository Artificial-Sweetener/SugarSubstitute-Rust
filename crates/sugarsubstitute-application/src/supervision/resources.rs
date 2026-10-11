// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own connection-bound leases and profile exclusion without native handle authority.
use super::resource_state::*;
use std::collections::BTreeMap;
use sugarsubstitute_contracts::lifecycle::*;

/// Observe bounded retained policy state, independently of actual native handles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoordinatorSnapshot {
    /// Registered run records, including unsettled disconnected runs.
    pub runs: usize,
    /// Pending/live native acquisition records, excluding settled records.
    pub resources: usize,
    /// Pending and ready connection-bound leases.
    pub leases: usize,
    /// Outstanding preparation requests whose outcome is not yet terminal.
    pub outstanding_requests: usize,
    /// Exclusive profiles whose actual desktop/browser settlement is unobserved.
    pub retained_profiles: usize,
    /// Run-local references to current unsettled resource generations, bounded by 4096.
    pub tracked_resource_refs: usize,
}
/// Serialize resource policy at one application owner; methods perform no I/O.
///
/// Trusted registration/observations are separate from decoded requests. Neither
/// path authenticates itself. The composing adapter validates peer identity,
/// resolves/trusts managed paths and retains native claims/process entries outside
/// execution futures. Cancellation or abandonment of an `OwnedJob` cannot remove
/// these records. Only actual settlement observations release overlap protection.
pub struct ResourceCoordinator {
    pub(super) owner: CoordinatorKey,
    pub(super) runs: BTreeMap<InstanceId, RunRecord>,
    pub(super) resources: BTreeMap<ResourceId, ResourceRecord>,
    pub(super) instance_highwater: u64,
    pub(super) next_resource: u64,
    pub(super) next_lease: u64,
}
impl ResourceCoordinator {
    /// Start an empty coordinator with a fresh native-authenticated owner epoch.
    pub fn new(owner: CoordinatorKey) -> Self {
        Self {
            owner,
            runs: BTreeMap::new(),
            resources: BTreeMap::new(),
            instance_highwater: 0,
            next_resource: 0,
            next_lease: 0,
        }
    }
    /// Register authenticated run/profile facts before any backend acquisition.
    ///
    /// # Errors
    /// Rejects foreign/stale runs, exclusive profile conflicts and run overload
    /// without starting a backend. A new generation cannot overlap an unsettled
    /// old run. Retired instance IDs cannot be resurrected after their record evicts.
    pub fn register_run(
        &mut self,
        registration: ClientRegistration,
    ) -> Result<(), LifecycleRejection> {
        let run = registration.run;
        if run.owner != self.owner {
            return Err(LifecycleRejection::ForeignOwner);
        }
        if let Some(previous) = self.runs.get(&run.instance) {
            if previous.registration == registration && previous.connected {
                return Ok(());
            }
            if run.generation <= previous.registration.run.generation {
                return Err(LifecycleRejection::StaleIdentity);
            }
            if !previous.settled {
                return Err(LifecycleRejection::WrongPhase);
            }
        } else if run.instance.get() <= self.instance_highwater {
            return Err(LifecycleRejection::StaleIdentity);
        }
        if self
            .runs
            .values()
            .any(|record| !record.settled && record.registration.profile == registration.profile)
        {
            return Err(LifecycleRejection::ProfileContention);
        }
        if !self.runs.contains_key(&run.instance) && self.runs.len() >= MAX_RUNS {
            let retired = self.runs.iter().find(|(_, record)| record.settled).map(|(id, _)| *id);
            if let Some(retired) = retired {
                self.runs.remove(&retired);
            } else {
                return Err(LifecycleRejection::Overloaded);
            }
        }
        self.instance_highwater = self.instance_highwater.max(run.instance.get());
        self.runs.insert(
            run.instance,
            RunRecord {
                registration,
                connected: true,
                settled: false,
                highwater: 0,
                pending: BTreeMap::new(),
                history: Default::default(),
                resources: BTreeMap::new(),
            },
        );
        Ok(())
    }
    /// Close one authenticated connection and release its leases, not its profile.
    ///
    /// Effects are bounded by the 64 resources. Losing this client never stops a
    /// resource still leased by another run. Browser/profile ownership remains
    /// protected until `settle_run` receives actual native settlement evidence.
    pub fn disconnect_run(
        &mut self,
        run: RunKey,
        connection: ConnectionId,
    ) -> Result<ResourceTransition, LifecycleRejection> {
        self.check_run(run, connection)?;
        let mut transition = ResourceTransition::default();
        let leases: Vec<_> = self
            .resources
            .values()
            .flat_map(|r| r.leases.values())
            .filter(|lease| lease.run == run && lease.connection == connection)
            .copied()
            .collect();
        for lease in leases {
            self.release_lease(lease, &mut transition)?;
        }
        if let Some(record) = self.runs.get_mut(&run.instance) {
            record.connected = false;
        }
        Ok(transition)
    }
    /// Acknowledge actual desktop/browser settlement after connection closure.
    ///
    /// # Errors
    /// Cancellation, process root exit and socket loss are insufficient. Rejects
    /// a connected run, live leases or unsettled last-client resource cleanup.
    /// The trusted adapter separately proves all profile/native work actually stopped.
    pub fn settle_run(&mut self, run: RunKey) -> Result<(), LifecycleRejection> {
        self.check_identity(run)?;
        let record = self.runs.get(&run.instance).ok_or(LifecycleRejection::StaleIdentity)?;
        if record.connected
            || self
                .resources
                .values()
                .any(|resource| resource.leases.values().any(|lease| lease.run == run))
        {
            return Err(LifecycleRejection::WrongPhase);
        }
        if record.resources.values().any(|key| {
            self.resources.get(&key.id).is_some_and(|resource| {
                resource.key == *key
                    && matches!(resource.phase, ResourcePhase::Stopping | ResourcePhase::Uncertain)
            })
        }) {
            return Err(LifecycleRejection::WrongPhase);
        }
        if let Some(record) = self.runs.get_mut(&run.instance) {
            record.settled = true;
        }
        Ok(())
    }
    /// Borrow the first retained original native failure for this exact generation.
    pub fn failure(&self, resource: ResourceKey) -> Option<&LifecycleFailure> {
        self.resources
            .get(&resource.id)
            .filter(|r| r.key == resource)
            .and_then(|r| r.failure.as_ref())
    }
    /// Transfer and acknowledge the retained failure after actual native settlement.
    ///
    /// Until transferred, the record cannot be evicted or reused for another
    /// generation; the original typed cause cannot disappear on a later request.
    pub fn take_failure(
        &mut self,
        key: ResourceKey,
    ) -> Result<Option<LifecycleFailure>, LifecycleRejection> {
        let resource = self
            .resources
            .get_mut(&key.id)
            .filter(|r| r.key == key)
            .ok_or(LifecycleRejection::StaleIdentity)?;
        if resource.phase != ResourcePhase::Settled {
            return Err(LifecycleRejection::WrongPhase);
        }
        Ok(resource.failure.take())
    }
    /// Inspect retained policy phase without inferring native completion.
    pub fn resource_phase(&self, resource: ResourceKey) -> Option<ResourcePhase> {
        self.resources.get(&resource.id).filter(|r| r.key == resource).map(|r| r.phase)
    }
    /// Count real retained records; none are released by a cancelled execution job.
    pub fn snapshot(&self) -> CoordinatorSnapshot {
        CoordinatorSnapshot {
            runs: self.runs.len(),
            resources: self
                .resources
                .values()
                .filter(|r| r.phase != ResourcePhase::Settled)
                .count(),
            leases: self.resources.values().map(|r| r.leases.len()).sum(),
            outstanding_requests: self.runs.values().map(|r| r.pending.len()).sum(),
            retained_profiles: self.runs.values().filter(|r| !r.settled).count(),
            tracked_resource_refs: self.runs.values().map(|r| r.resources.len()).sum(),
        }
    }
    pub(super) fn check_identity(&self, run: RunKey) -> Result<(), LifecycleRejection> {
        if run.owner != self.owner {
            return Err(LifecycleRejection::ForeignOwner);
        }
        if self.runs.get(&run.instance).is_none_or(|record| record.registration.run != run) {
            return Err(LifecycleRejection::StaleIdentity);
        }
        Ok(())
    }
    pub(super) fn check_run(
        &self,
        run: RunKey,
        connection: ConnectionId,
    ) -> Result<(), LifecycleRejection> {
        self.check_identity(run)?;
        if self
            .runs
            .get(&run.instance)
            .is_none_or(|record| !record.connected || record.registration.connection != connection)
        {
            return Err(LifecycleRejection::StaleIdentity);
        }
        Ok(())
    }
}
