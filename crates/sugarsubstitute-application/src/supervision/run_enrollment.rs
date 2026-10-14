// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reserve bounded startup identities separately from actual connected peer registration.
use super::{ResourceCoordinator, resource_state::*};
use std::collections::BTreeMap;
use sugarsubstitute_contracts::lifecycle::*;

impl ResourceCoordinator {
    /// Reserve the current run/profile before fallible child initialization.
    /// Exact live repeats retain their existing phase and incident markers.
    /// Enrollment authenticates no peer and permits no resource request.
    ///
    /// # Errors
    /// Rejects foreign/stale identities, changed enrollment, overlapping generations,
    /// exclusive profile conflicts and the original64-run bound without mutation.
    /// Settled instance identities require a strictly newer generation; evicted IDs
    /// cannot be resurrected. The adapter must prove actual capsule settlement.
    pub fn enroll_run(&mut self, enrollment: RunEnrollment) -> Result<(), LifecycleRejection> {
        let run = enrollment.run;
        if run.owner != self.owner {
            return Err(LifecycleRejection::ForeignOwner);
        }
        if let Some(previous) = self.runs.get(&run.instance) {
            if previous.enrollment == enrollment && !previous.settled {
                return Ok(());
            }
            if run.generation <= previous.enrollment.run.generation {
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
            .any(|record| !record.settled && record.enrollment.profile == enrollment.profile)
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
                enrollment,
                incident_seed_admitted: false,
                incident_failure_admitted: false,
                connection: ConnectionPhase::Admitted,
                settled: false,
                highwater: 0,
                pending: BTreeMap::new(),
                history: Default::default(),
                resources: BTreeMap::new(),
            },
        );
        Ok(())
    }

    /// Record the trusted adapter's native authentication for this exact enrollment.
    /// The caller authenticates the retained peer before invoking this method.
    /// Exact connected repeats are idempotent and never reset request/seed history.
    ///
    /// # Errors
    /// Rejects missing/foreign/stale enrollment, changed launch/profile/connection,
    /// disconnected reconnect and actual settled runs. This method never enrolls.
    pub fn register_run(
        &mut self,
        registration: ClientRegistration,
    ) -> Result<(), LifecycleRejection> {
        self.check_identity(registration.run)?;
        let record = self
            .runs
            .get_mut(&registration.run.instance)
            .ok_or(LifecycleRejection::StaleIdentity)?;
        if record.enrollment.launch != registration.launch
            || record.enrollment.profile != registration.profile
            || record.settled
        {
            return Err(LifecycleRejection::StaleIdentity);
        }
        match record.connection {
            ConnectionPhase::Admitted => {
                record.connection = ConnectionPhase::Connected(registration.connection);
                Ok(())
            }
            ConnectionPhase::Connected(current) if current == registration.connection => Ok(()),
            ConnectionPhase::Connected(_) | ConnectionPhase::Disconnected(_) => {
                Err(LifecycleRejection::StaleIdentity)
            }
        }
    }
}
