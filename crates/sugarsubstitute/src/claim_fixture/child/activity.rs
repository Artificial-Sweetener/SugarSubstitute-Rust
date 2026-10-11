// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume actual claim facts and admit one finite child decision per native turn.
use super::*;
impl Child {
    fn drain(&mut self, max: NonZeroUsize) -> Result<bool> {
        let Some(client) = &self.client else { return Ok(false) };
        let batch = client.drain_completions(max)?;
        let mut changed = !batch.notifications.is_empty();
        if self.claims.consume(&batch)? != 0 {
            self.pending = false;
        }
        let native_job_id = JobId::new(1)?;
        if batch.notifications.iter().any(|notice| notice.job_id == native_job_id)
            && let Some(permit) = self.permit.take()
        {
            self.protocol = Some(permit.cancel_unbound());
        }
        let mut failures = failure::TurnFailures::new();
        for _ in 0..3 {
            let Some(event) = self.claims.take_observation() else { break };
            changed = true;
            if let Err(cause) = self.observe(event) {
                failures.record("child_claim_observation", cause);
            }
        }
        if let Some(job) = &self.protocol {
            self.protocol_notice |= batch
                .notifications
                .iter()
                .any(|notice| (notice.scope_id, notice.job_id) == (job.scope_id(), job.spec().id));
        }
        if self.protocol_notice {
            let completion = self.protocol.as_mut().and_then(OwnedJob::try_take_completion);
            if let Some(completion) = completion {
                let job = self.protocol.take().ok_or("Claim child event ownership disappeared")?;
                self.protocol_notice = false;
                if completion.scope_id != job.scope_id() || completion.spec != job.spec() {
                    failures.record(
                        "child_terminal_identity",
                        "Claim child terminal identity changed".into(),
                    );
                }
                match failure::terminal(completion.outcome) {
                    Ok(report) => {
                        if report.correlation != self.options.correlation
                            || !(report.inheritance_cleared
                                && report.ready_signalled
                                && report.response_signalled
                                && report.release_observed)
                        {
                            failures.record(
                                "child_protocol_validation",
                                "Claim child protocol lacks causal inheritance/response/release"
                                    .into(),
                            );
                        }
                        self.protocol_report = Some(report);
                        if self.options.role == Role::ActiveHolder {
                            self.phase = Phase::Closing;
                            self.claims.close_admission();
                        }
                    }
                    Err(cause) => failures.record("child_event_terminal", cause),
                }
                changed = true;
            }
        }
        failures.finish()?;
        Ok(changed)
    }
    fn observe(&mut self, event: ClaimObservation) -> Result<()> {
        match event {
            ClaimObservation::CoordinatorClaimed(key) => {
                if self.key.is_some() {
                    return Err("Claim child received duplicate coordinator acquisition".into());
                }
                self.key = Some(key);
                let registry = WindowsProcessRegistry::new(key)?;
                self.claims.bind_process_registry(&registry)?;
                self.registry = Some(registry);
                self.phase = match self.options.role {
                    Role::ActiveHolder => Phase::Protocol,
                    Role::BackendContender | Role::BackendSuccess => Phase::Resolving,
                    _ => return Err("Coordinator contender unexpectedly acquired exclusion".into()),
                };
            }
            ClaimObservation::LaunchResolved(_) => {
                if self.phase != Phase::Resolving || self.resolution.is_none() {
                    return Err("Claim child resolution is premature".into());
                }
                self.phase = Phase::Acquiring;
            }
            ClaimObservation::BackendClaimed(resource) => {
                if Some(resource) != self.resource {
                    return Err("Claim child backend identity changed".into());
                }
                self.backend_acquired = true;
                if self.options.role != Role::BackendSuccess {
                    return Err("Backend contender unexpectedly acquired exclusion".into());
                }
                self.phase = Phase::Releasing;
            }
            ClaimObservation::BackendReleased(resource) => {
                if Some(resource) != self.resource {
                    return Err("Claim child backend release identity changed".into());
                }
                self.backend_released = self.backend_acquired;
                self.resource = None;
                self.phase = Phase::Closing;
                self.claims.close_admission();
            }
            ClaimObservation::ResolutionRetired(_) => self.resolution = None,
            ClaimObservation::NativeClaimsClosed => {
                self.claims_closed = true;
                self.phase = Phase::Protocol;
            }
            ClaimObservation::Failed { target, cause } => {
                let target_matches = match (self.options.role, &target) {
                    (
                        Role::CoordinatorContender | Role::ActivePredecessor,
                        ClaimTarget::Coordinator,
                    ) => true,
                    (Role::BackendContender, ClaimTarget::Backend(resource)) => {
                        Some(*resource) == self.resource
                    }
                    _ => false,
                };
                if self.expected_failure.is_some()
                    || !target_matches
                    || self.options.role.expected_failure() != Some(cause.operation())
                {
                    return Err(Box::new(cause));
                }
                self.expected_failure = Some(cause.operation());
                self.phase = Phase::Closing;
            }
            ClaimObservation::Stopped { .. } => {
                return Err("Claim child finite job stopped before required evidence".into());
            }
        }
        Ok(())
    }
    /// Cleanup runs through the same finite job ports, without renewing its shutdown budget.
    fn cleanup(&mut self) -> Result<bool> {
        if !self.initialization_admitted || self.pending || self.claims_closed {
            return Ok(false);
        }
        self.cleanup_context()?;
        let spec = self.spec(CLAIM_OPERATION_BYTES, true)?;
        let client = self.client.as_ref().ok_or("Claim child client missing")?;
        let scope = self.cleanup_scope.as_ref().ok_or("Claim child cleanup scope missing")?;
        let result = if let Some(resource) = self.resource {
            self.claims
                .submit_backend_release(
                    client,
                    scope,
                    spec,
                    self.registry.as_ref().ok_or("Claim child registry missing")?,
                    resource,
                )
                .map(|_| ())
        } else if let Some(ticket) = &self.resolution {
            self.claims.submit_retire_resolution(client, scope, spec, ticket).map(|_| ())
        } else {
            self.claims.close_admission();
            self.claims
                .submit_close(
                    client,
                    scope,
                    spec,
                    self.registry.as_mut().ok_or("Claim child registry missing")?,
                )
                .map(|_| ())
        };
        match result {
            Ok(()) => {
                self.pending = true;
                Ok(true)
            }
            Err(ClaimAdmissionError::Busy) => Ok(false),
            Err(cause) => Err(Box::new(cause)),
        }
    }
    fn step(&mut self) -> Result<bool> {
        if self.closing || matches!(self.phase, Phase::Closing | Phase::Releasing) {
            return self.cleanup();
        }
        if self.pending {
            return Ok(false);
        }
        match self.phase {
            Phase::Resolving if self.resolution.is_none() => {
                let configuration = self.configuration()?;
                let spec = self.spec(CLAIM_OPERATION_BYTES, false)?;
                let ticket = self.claims.submit_resolve_launch(
                    self.client.as_ref().ok_or("Claim child client missing")?,
                    self.scope.as_ref().ok_or("Claim child scope missing")?,
                    spec,
                    configuration,
                )?;
                self.resolution = Some(ticket);
                self.pending = true;
                Ok(true)
            }
            Phase::Acquiring => {
                let resource = ResourceKey {
                    owner: self.key.ok_or("Claim child coordinator missing")?,
                    id: ResourceId::new(1)?,
                    generation: ResourceGeneration::new(1)?,
                };
                let spec = self.spec(CLAIM_OPERATION_BYTES, false)?;
                self.claims.submit_backend_claim(
                    self.client.as_ref().ok_or("Claim child client missing")?,
                    self.scope.as_ref().ok_or("Claim child scope missing")?,
                    spec,
                    resource,
                    self.resolution.as_ref().ok_or("Claim child resolution missing")?,
                )?;
                self.resource = Some(resource);
                self.resolution = None;
                self.pending = true;
                Ok(true)
            }
            Phase::Protocol if self.protocol.is_none() && self.protocol_report.is_none() => {
                let permit = self.permit.take().ok_or("Claim child late-bind permit missing")?;
                let events = self
                    .options
                    .events
                    .take()
                    .ok_or("Claim child inherited roles already transferred")?;
                self.protocol = Some(permit.bind(events));
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    pub(super) fn advance(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        let mut changed = self.drain(max)?;
        if !self.deadline_reported
            && self.native_deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            self.deadline_reported = true;
            return Err(
                "Claim child exceeded original admitted deadline before native settlement".into()
            );
        }
        changed |= self.step()?;
        if changed {
            self.notify()?;
        }
        Ok(if self.closing || (self.claims_closed && self.protocol_report.is_some()) {
            HeadlessStep::Shutdown
        } else {
            HeadlessStep::Active
        })
    }
}
