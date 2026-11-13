// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume bounded actual facts before making one campaign decision.
use super::*;
impl Fixture {
    fn drain(&mut self, max: NonZeroUsize) -> Result<bool> {
        let Some(client) = &self.client else { return Ok(false) };
        let batch = client.drain_completions(max)?;
        let table_changed = crate::process_fixture::registry::poll(
            &mut self.registry_creation,
            &mut self.registry,
            self.closing,
        )?;
        if table_changed && self.registry_binding_pending {
            self.registry_binding_pending = false;
            if !self.closing {
                self.claims.bind_process_registry(
                    self.registry.as_ref().ok_or("Actual registry constructor lacks success")?,
                )?;
                self.phase = Phase::Resolve;
            }
        }

        let mut changed = table_changed || !batch.notifications.is_empty();
        if self.claims.consume(&batch)? != 0 {
            self.claims_pending = false;
        }
        if let Some(registry) = &mut self.registry {
            registry.consume(&batch)?;
        }
        self.setup_notice |= self.setup.as_ref().is_some_and(|job| {
            batch
                .notifications
                .iter()
                .any(|n| (n.scope_id, n.job_id) == (job.scope_id(), job.spec().id))
        });
        if let Some(case) = &mut self.child {
            if let Some(session) = &case.session {
                for n in &batch.notifications {
                    if (n.scope_id, n.job_id)
                        == (session.completion.scope_id(), session.completion.spec().id)
                    {
                        case.event_notice = Some(n.kind);
                    }
                }
            }
            case.read_notice |= case.read.as_ref().is_some_and(|job| {
                batch
                    .notifications
                    .iter()
                    .any(|n| (n.scope_id, n.job_id) == (job.scope_id(), job.spec().id))
            });
        }
        for managed in [true, false] {
            if let Some(family) = self.family_mut(managed) {
                for n in &batch.notifications {
                    if family.cleanup_job == Some(n.job_id) {
                        family.cleanup_job = None;
                        family.cleanup_requested = true;
                    }
                    if family.member_job == Some(n.job_id) {
                        family.member_job = None;
                        family.cleanup_requested = true;
                    }
                    if n.job_id == family.creation || n.job_id == family.root_watch {
                        family.cleanup_requested = true;
                    }
                }
            }
        }
        let mut failures = failure::TurnFailures::new();
        for _ in 0..3 {
            let Some(event) = self.claims.take_observation() else { break };
            changed = true;
            if let Err(cause) = self.observe_claim(event) {
                failures.record("parent_claim_observation", cause);
            }
        }
        for managed in [true, false] {
            let Some(key) =
                self.family(managed).filter(|f| !f.progress.retired).map(|f| f.ticket.process())
            else {
                continue;
            };
            for _ in 0..5 {
                let Some(event) = self
                    .registry
                    .as_mut()
                    .ok_or("Claim registry missing")?
                    .take_observation(key)?
                else {
                    break;
                };
                changed = true;
                let family = self.family_mut(managed).ok_or("Claim family missing")?;
                // Transfer settlement independently of a previous diagnostic failure.
                if matches!(event, RegistryObservation::FamilySettled { .. }) {
                    family.settlement_transferred = true;
                }
                if let Err(cause) = family.progress.observe(event) {
                    failures.record("parent_registry_observation", cause);
                }
            }
        }
        if let Err(cause) = self.deliver() {
            failures.record("parent_typed_delivery", cause);
        }
        failures.finish()?;
        Ok(changed)
    }
    fn observe_claim(&mut self, event: ClaimObservation) -> Result<()> {
        match event {
            ClaimObservation::CoordinatorClaimed(key) => {
                if self.key.is_some() {
                    return Err("Duplicate claim coordinator fact".into());
                }
                self.key = Some(key);
                self.begin_registry(key)?;
                self.registry_binding_pending = true;
                self.phase = Phase::Registry;
            }
            ClaimObservation::LaunchResolved(_) => {
                if self.resolution.is_none() || self.phase != Phase::Resolve {
                    return Err("Unexpected claim resolution fact".into());
                }
                self.phase = Phase::Acquire;
            }
            ClaimObservation::BackendClaimed(resource) => {
                if self.resource != Some(resource) || self.phase != Phase::Acquire {
                    return Err("Unexpected backend acquisition fact".into());
                }
                self.phase = Phase::CoordinatorContender;
            }
            ClaimObservation::BackendReleased(resource) => {
                if self.resource != Some(resource) {
                    return Err("Backend release identity changed".into());
                }
                self.resource = None;
                self.managed_released = true;
            }
            ClaimObservation::ResolutionRetired(_) => self.resolution = None,
            ClaimObservation::NativeClaimsClosed => {
                self.claims_closed = true;
                self.phase = Phase::Done;
            }
            ClaimObservation::Failed { cause, .. } => return Err(Box::new(cause)),
            ClaimObservation::Stopped { .. } => {
                return Err("Claim campaign job stopped before required native facts".into());
            }
        }
        Ok(())
    }
    pub(super) fn advance(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        let mut changed = self.drain(max)?;
        if !self.deadline_observed && Instant::now() >= self.deadline {
            self.deadline_observed = true;
            return Err("Claim campaign exceeded original sixty-second deadline".into());
        }
        if self.closing {
            changed |= self.closing_step()?;
        } else {
            changed |= self.step()?;
        }
        if changed {
            self.notify()?;
        }
        Ok(if self.closing || self.phase == Phase::Done {
            HeadlessStep::Shutdown
        } else {
            HeadlessStep::Active
        })
    }
}
