// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sequence causal cross-process contention around actual managed settlement.
use super::*;
impl Fixture {
    pub(super) fn step(&mut self) -> Result<bool> {
        let mut changed = self.drive_child()?;
        match self.phase {
            Phase::Setup => {
                if self.setup_finished {
                    self.phase = Phase::Initialize;
                    changed = true;
                } else {
                    changed |= self.setup_workspace()?;
                }
            }
            Phase::Initialize if !self.claims_pending && !self.initialization_admitted => {
                let spec = self.spec(JobClass::InfrastructureLifecycle, CLAIM_POOL_BYTES, true)?;
                self.claims.submit_initialize(
                    self.client.as_ref().ok_or("Claim client missing")?,
                    self.scope.as_ref().ok_or("Claim scope missing")?,
                    spec,
                    self.options.workspace.join("installation-a"),
                )?;
                self.initialization_admitted = true;
                self.claims_pending = true;
                changed = true;
            }
            Phase::Resolve if !self.claims_pending && self.resolution.is_none() => {
                let configuration = self.configuration()?;
                let spec =
                    self.spec(JobClass::InfrastructureLifecycle, CLAIM_OPERATION_BYTES, true)?;
                self.resolution = Some(self.claims.submit_resolve_launch(
                    self.client.as_ref().ok_or("Claim client missing")?,
                    self.scope.as_ref().ok_or("Claim scope missing")?,
                    spec,
                    configuration,
                )?);
                self.claims_pending = true;
                changed = true;
            }
            Phase::Acquire if !self.claims_pending => {
                let resource = ResourceKey {
                    owner: self.key.ok_or("Claim owner missing")?,
                    id: ResourceId::new(1)?,
                    generation: ResourceGeneration::new(1)?,
                };
                let spec =
                    self.spec(JobClass::InfrastructureLifecycle, CLAIM_OPERATION_BYTES, true)?;
                self.claims.submit_backend_claim(
                    self.client.as_ref().ok_or("Claim client missing")?,
                    self.scope.as_ref().ok_or("Claim scope missing")?,
                    spec,
                    resource,
                    self.resolution.as_ref().ok_or("Claim resolution missing")?,
                )?;
                self.resolution = None;
                self.resource = Some(resource);
                self.claims_pending = true;
                changed = true;
            }
            Phase::CoordinatorContender => {
                changed |= self.child_phase(
                    Role::CoordinatorContender,
                    "installation-a",
                    Phase::BackendBefore,
                )?
            }
            Phase::BackendBefore => {
                changed |= self.child_phase(
                    Role::BackendContender,
                    "installation-b",
                    Phase::LaunchManaged,
                )?
            }
            Phase::LaunchManaged => {
                self.launch_managed()?;
                self.phase = Phase::ManagedExit;
                changed = true;
            }
            Phase::ManagedExit => {
                if self.managed.as_ref().is_some_and(|f| f.progress.exited == Some(37)) {
                    let spec =
                        self.spec(JobClass::InfrastructureLifecycle, CLAIM_OPERATION_BYTES, true)?;
                    let result = self.claims.submit_backend_release(
                        self.client.as_ref().ok_or("Claim client missing")?,
                        self.scope.as_ref().ok_or("Claim scope missing")?,
                        spec,
                        self.registry.as_ref().ok_or("Claim registry missing")?,
                        self.resource.ok_or("Claim resource missing")?,
                    );
                    match result {
                        Err(ClaimAdmissionError::Busy) => self.premature_release_rejected = true,
                        Err(cause) => return Err(Box::new(cause)),
                        Ok(_) => {
                            self.claims_pending = true;
                            return Err("Claim released exclusion before family settlement".into());
                        }
                    }
                    self.root_exit_before_release = true;
                    self.begin_stop(true)?;
                    self.phase = Phase::BackendAfterExit;
                    changed = true;
                }
            }
            Phase::BackendAfterExit => {
                changed |= self.child_phase(
                    Role::BackendContender,
                    "installation-b",
                    Phase::ManagedSettlement,
                )?
            }
            Phase::ManagedSettlement => {
                changed |= self.family_step(true, true)?;
                if self
                    .managed
                    .as_ref()
                    .is_some_and(|f| f.progress.settled && f.settlement_transferred)
                {
                    self.phase = Phase::ManagedRelease;
                    changed = true;
                }
            }
            Phase::ManagedRelease => {
                if self.resource.is_some() {
                    changed |= self.claims_cleanup()?;
                } else {
                    changed |= self.retire_family(true)?;
                    if self.managed.as_ref().is_some_and(|f| f.progress.complete()) {
                        self.phase = Phase::BackendSuccess;
                        changed = true;
                    }
                }
            }
            Phase::BackendSuccess => {
                changed |=
                    self.child_phase(Role::BackendSuccess, "installation-b", Phase::ActiveHolder)?
            }
            Phase::ActiveHolder => {
                changed |= self.start_child(Role::ActiveHolder, "installation-stale", true)?;
                if self.child.as_ref().is_some_and(|case| case.responded) {
                    let case = self.child.as_mut().ok_or("Claim holder missing")?;
                    case.session
                        .as_ref()
                        .ok_or("Claim holder event owner missing")?
                        .control
                        .cancel();
                    case.event_cancel_requested = true;
                    self.begin_stop(false)?;
                    self.phase = Phase::ActiveKill;
                    changed = true;
                }
            }
            Phase::ActiveKill => {
                // Original ten-second grace is required; event cancellation does
                // not signal Release or prove that the contained process stopped.
                if self.finish_child()? {
                    self.phase = Phase::Predecessor;
                    changed = true;
                }
            }
            Phase::Predecessor => {
                changed |= self.child_phase(
                    Role::ActivePredecessor,
                    "installation-stale",
                    Phase::PredecessorAgain,
                )?
            }
            Phase::PredecessorAgain => {
                changed |=
                    self.child_phase(Role::ActivePredecessor, "installation-stale", Phase::Close)?
            }
            Phase::Close => changed |= self.claims_cleanup()?,
            _ => (),
        }
        // Retain the managed claim throughout the independently run contender.
        // Its immutable native stop boundaries are still consumed on every wake.
        if self.phase == Phase::BackendAfterExit {
            changed |= self.family_step(true, false)?;
        }
        Ok(changed)
    }
    fn child_phase(&mut self, role: Role, installation: &str, next: Phase) -> Result<bool> {
        let mut changed = self.start_child(role, installation, false)?;
        if self.finish_child()? {
            self.phase = next;
            changed = true;
        }
        Ok(changed)
    }
}
