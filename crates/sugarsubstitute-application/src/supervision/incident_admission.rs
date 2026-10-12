// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain incident admission alongside the authoritative bounded run generation.
use super::{FailedDesktopFact, ResourceCoordinator};
use sugarsubstitute_contracts::lifecycle::{LifecycleRejection, RunKey};

impl ResourceCoordinator {
    /// Mark one current registered seed after incident-owner preflight completes.
    pub(crate) fn admit_incident_seed(&mut self, run: RunKey) -> Result<(), LifecycleRejection> {
        self.check_identity(run)?;
        let record = self.runs.get_mut(&run.instance).ok_or(LifecycleRejection::StaleIdentity)?;
        if record.incident_seed_admitted {
            return Err(LifecycleRejection::StaleIdentity);
        }
        record.incident_seed_admitted = true;
        Ok(())
    }

    /// Mark the accepted failed exit once; classification stays with its transaction.
    pub(crate) fn admit_failed_incident(
        &mut self,
        fact: FailedDesktopFact,
    ) -> Result<(), LifecycleRejection> {
        let run = fact.run();
        self.check_identity(run)?;
        let record = self.runs.get_mut(&run.instance).ok_or(LifecycleRejection::StaleIdentity)?;
        if !record.incident_seed_admitted {
            return Err(LifecycleRejection::WrongPhase);
        }
        if record.incident_failure_admitted {
            return Err(LifecycleRejection::StaleIdentity);
        }
        record.incident_failure_admitted = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::supervision::DesktopExitTransaction;
    use sugarsubstitute_contracts::{configuration::ProfileId, lifecycle::*, startup::LaunchId};
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn run() -> Result<RunKey, IdentityError> {
        Ok(RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        })
    }
    fn register(resources: &mut ResourceCoordinator, run: RunKey) -> TestResult {
        resources.register_run(ClientRegistration {
            run,
            launch: LaunchId::new(1)?,
            profile: ProfileId::new("incident-admission".into())?,
            connection: ConnectionId::new(1)?,
        })?;
        Ok(())
    }
    fn fact(run: RunKey) -> Result<FailedDesktopFact, Box<dyn std::error::Error>> {
        let process = ProcessKey { owner: run.owner, id: ProcessId::new(1)? };
        let mut exit = DesktopExitTransaction::new(run, process)?;
        exit.observe(DesktopObservation::Exited { process, code: 0 }).map_err(|e| e.reason)?;
        Ok(exit.failed_incident()?)
    }
    #[test]
    fn incident_markers_require_seed_then_actual_failed_exit_and_advance_once() -> TestResult {
        let run = run()?;
        let mut resources = ResourceCoordinator::new(run.owner);
        register(&mut resources, run)?;
        let fact = fact(run)?;
        assert_eq!(resources.admit_failed_incident(fact), Err(LifecycleRejection::WrongPhase));
        resources.admit_incident_seed(run)?;
        assert_eq!(resources.admit_incident_seed(run), Err(LifecycleRejection::StaleIdentity));
        resources.admit_failed_incident(fact)?;
        assert_eq!(resources.admit_failed_incident(fact), Err(LifecycleRejection::StaleIdentity));
        Ok(())
    }
    #[test]
    fn foreign_or_stale_identity_cannot_poison_current_incident_markers() -> TestResult {
        let run = run()?;
        let mut resources = ResourceCoordinator::new(run.owner);
        register(&mut resources, run)?;
        let mut stale = run;
        stale.generation = run.generation.next()?;
        assert_eq!(resources.admit_incident_seed(stale), Err(LifecycleRejection::StaleIdentity));
        assert_eq!(
            resources.admit_failed_incident(fact(stale)?),
            Err(LifecycleRejection::StaleIdentity)
        );
        let mut foreign = run;
        foreign.owner.epoch = CoordinatorEpoch::new([3; 16])?;
        assert_eq!(resources.admit_incident_seed(foreign), Err(LifecycleRejection::ForeignOwner));
        resources.admit_incident_seed(run)?;
        resources.admit_failed_incident(fact(run)?)?;
        Ok(())
    }
}
