// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Track exact case evidence and consume original cleanup boundaries once.
use super::*;
use sugarsubstitute_contracts::lifecycle::{ProcessKey, ShutdownBudget};
use sugarsubstitute_supervision::RegistryObservation;

pub(crate) struct CaseProgress {
    pub(crate) expected: u32,
    pub(crate) process: Option<ProcessKey>,
    pub(crate) started: bool,
    pub(crate) exited: Option<u32>,
    pub(crate) stop: Option<(ShutdownId, ShutdownBudget)>,
    pub(crate) settled: bool,
    pub(crate) retired: bool,
}
impl CaseProgress {
    pub(crate) const fn new(expected: u32) -> Self {
        Self {
            expected,
            process: None,
            started: false,
            exited: None,
            stop: None,
            settled: false,
            retired: false,
        }
    }
    pub(crate) fn bind(&mut self, key: ProcessKey) -> Result<()> {
        if self.process.is_some() {
            return Err("Fixture case already has a retained family".into());
        }
        self.process = Some(key);
        Ok(())
    }
    pub(crate) fn observe(&mut self, observation: RegistryObservation) -> Result<()> {
        let key = match &observation {
            RegistryObservation::ContainedStarted { process }
            | RegistryObservation::RootExited { process, .. }
            | RegistryObservation::RootObservationRetired { process, .. }
            | RegistryObservation::FamilySettled { process, .. }
            | RegistryObservation::Failed { process, .. } => *process,
        };
        if self.process != Some(key) || self.retired {
            return Err("Fixture observation has a foreign or retired identity".into());
        }
        match observation {
            RegistryObservation::ContainedStarted { .. } => self.started = true,
            RegistryObservation::RootExited { code, .. } => {
                self.exited = Some(code);
                if code != self.expected {
                    return Err("Fixture observed an unexpected actual exit code".into());
                }
            }
            RegistryObservation::RootObservationRetired { .. } => {
                return Err("Immediate-exit fixture requires an actual exit observation".into());
            }
            RegistryObservation::FamilySettled { shutdown: observed, .. } => {
                if observed != self.stop.map(|stop| stop.0) || observed.is_none() {
                    return Err("Fixture settlement has a different stop transaction".into());
                }
                self.settled = true;
            }
            RegistryObservation::Failed { error, .. } => {
                tracing::error!(operation = "registry_fixture_observation", registry_process_id = key.id.get(),
                    stage = error.stage(), owner_panic = error.contains_owner_panic(),
                    omitted_causes = error.omitted_causes(), cause = %error,
                    native_cause = ?error.source(),
                    "Registry qualification retained an original native failure");
                for secondary in error.cleanup_causes() {
                    tracing::error!(operation = "registry_fixture_cleanup_cause", registry_process_id = key.id.get(),
                        stage = secondary.stage(), native_cause = ?secondary.source(),
                        "Registry qualification observed a secondary native cause");
                }
                return Err(Box::new(error));
            }
        }
        Ok(())
    }
    pub(crate) fn complete(&self) -> bool {
        self.started && self.exited == Some(self.expected) && self.settled && self.retired
    }
}

pub(crate) struct StopPlan {
    pub(crate) budget: ShutdownBudget,
    graceful: Instant,
    total: Instant,
    grace_observed: bool,
    total_observed: bool,
}
/// A consumed boundary requests finite cleanup; expiry never denotes settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BoundaryDue {
    pub(crate) cleanup: bool,
    pub(crate) expired: bool,
}
impl StopPlan {
    pub(crate) fn new(now: Moment, native_now: Instant) -> Result<Self> {
        Ok(Self {
            budget: ShutdownBudget::new(now)?,
            graceful: native_now
                .checked_add(Duration::from_secs(10))
                .ok_or("Fixture grace overflow")?,
            total: native_now
                .checked_add(Duration::from_secs(15))
                .ok_or("Fixture total overflow")?,
            grace_observed: false,
            total_observed: false,
        })
    }
    pub(crate) fn due(&mut self, now: Moment, settled: bool) -> BoundaryDue {
        let grace = !self.grace_observed && now >= self.budget.graceful_until;
        let total = !self.total_observed && now >= self.budget.settle_until;
        self.grace_observed |= grace;
        self.total_observed |= total;
        BoundaryDue { cleanup: grace || total, expired: total && !settled }
    }
    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        if !self.grace_observed {
            Some(self.graceful)
        } else if !self.total_observed {
            Some(self.total)
        } else {
            None
        }
    }
}
