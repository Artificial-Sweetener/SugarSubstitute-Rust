// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route actual adapter observations without deriving readiness from terminal metadata.
use super::*;
impl CoordinatorBackend {
    /// Consume one genuine claims observation routed by the actual authority.
    /// Unrelated observations are returned unchanged for their authoritative owner.
    pub(crate) fn claim_observation(
        &mut self,
        observation: ClaimObservation,
        resources: &mut BackendResources<'_>,
    ) -> Result<(ResourceTransition, Option<ClaimObservation>)> {
        let at = match &observation {
            ClaimObservation::BackendClaimed(key)
            | ClaimObservation::BackendReleased(key)
            | ClaimObservation::Failed { target: ClaimTarget::Backend(key), .. }
            | ClaimObservation::Stopped { target: ClaimTarget::Backend(key), .. } => {
                self.index(*key).ok()
            }
            ClaimObservation::LaunchResolved(ticket)
            | ClaimObservation::ResolutionRetired(ticket)
            | ClaimObservation::Failed { target: ClaimTarget::Resolution(ticket), .. }
            | ClaimObservation::Stopped { target: ClaimTarget::Resolution(ticket), .. } => {
                self.records.iter().position(|record| {
                    record.as_ref().is_some_and(|r| {
                        r.resolution
                            .as_ref()
                            .is_some_and(|original| original.same_resolution(ticket))
                    })
                })
            }
            _ => None,
        };
        let Some(at) = at else {
            return Ok((ResourceTransition::default(), Some(observation)));
        };
        self.validate(resources)?;
        self.begin_delivery()?;
        let record = self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
        let key = record.key;
        let mut transition = ResourceTransition::default();
        match observation {
            ClaimObservation::LaunchResolved(_) => {
                record.resolution_pending = false;
                tracing::info!(operation="backend_preparation_transition",resource=?key,stage="ResolutionTerminal",
                    "Original backend preparation advanced");
                if record.shutdown.is_none() {
                    record.budget.probe_until(resources.client.now()?)?;
                    let ticket =
                        record.resolution.as_ref().ok_or(LifecycleRejection::StaleIdentity)?;
                    let resolved =
                        resources.authority.claims().resolved_configuration(ticket)?.clone();
                    record.campaign = ReadinessCampaign::new(
                        BackendTarget::Managed(resolved.clone()),
                        record.budget,
                    )?;
                    record.resolved = Some(resolved);
                }
            }
            ClaimObservation::BackendClaimed(_) => {
                record.claim_pending = false;
                record.claimed = true;
                tracing::info!(operation="backend_preparation_transition",resource=?key,stage="ClaimTerminal",
                    "Original backend preparation advanced");
                transition = self.observe(ResourceObservation::ClaimAcquired(key))?;
            }
            ClaimObservation::BackendReleased(_) => {
                record.release_pending = false;
                record.released = true;
            }
            ClaimObservation::ResolutionRetired(_) => {
                record.resolution = None;
                record.resolution_retiring = false;
            }
            ClaimObservation::Failed { target, cause } => {
                match target {
                    ClaimTarget::Resolution(_) => {
                        record.resolution_pending = false;
                        tracing::info!(operation="backend_preparation_transition",resource=?key,stage="ResolutionTerminal",
                            "Original backend preparation advanced");
                    }
                    ClaimTarget::Backend(_) => {
                        if record.claim_pending {
                            tracing::info!(operation="backend_preparation_transition",resource=?key,stage="ClaimTerminal",
                                "Original backend preparation advanced");
                        }
                        record.claim_pending = false;
                        record.release_pending = false;
                    }
                    ClaimTarget::Coordinator => {}
                }
                transition = self.failed(key, Box::new(cause), resources)?;
            }
            ClaimObservation::Stopped { target, kind } => {
                match target {
                    ClaimTarget::Resolution(_) => {
                        record.resolution_pending = false;
                        tracing::info!(operation="backend_preparation_transition",resource=?key,stage="ResolutionTerminal",
                            "Original backend preparation advanced");
                    }
                    ClaimTarget::Backend(_) => {
                        if record.claim_pending {
                            tracing::info!(operation="backend_preparation_transition",resource=?key,stage="ClaimTerminal",
                                "Original backend preparation advanced");
                        }
                        record.claim_pending = false;
                        record.release_pending = false;
                    }
                    ClaimTarget::Coordinator => {}
                }
                transition = self.failed(key, Box::new(Stopped(kind)), resources)?;
            }
            other => return Ok((transition, Some(other))),
        }
        Ok((self.undelivered.take().unwrap_or(transition), None))
    }

    /// Advance one retained generation using the same immutable central batch.
    /// Claims/registry completion consumption belongs to CoordinatorAuthority;
    /// this method transfers only their actual cached family observations.
    pub(crate) fn activity(
        &mut self,
        key: ResourceKey,
        batch: &CompletionBatch,
        resources: &mut BackendResources<'_>,
    ) -> Result<ResourceTransition> {
        self.validate(resources)?;
        let at = self.index(key)?;
        self.begin_delivery()?;
        let mut transition = ResourceTransition::default();
        if let Some(ticket) = self.records[at].as_ref().and_then(|r| r.launch) {
            for _ in 0..5 {
                let observation = resources
                    .authority
                    .registry_mut()
                    .ok_or(LifecycleRejection::WrongPhase)?
                    .take_observation(ticket.process())?;
                let Some(observation) = observation else {
                    break;
                };
                match observation {
                    RegistryObservation::ContainedStarted { process } => {
                        tracing::info!(operation="backend_preparation_transition",resource=?key,stage="LaunchStarted",
                            "Original backend preparation advanced");
                        self.records[at]
                            .as_mut()
                            .ok_or(LifecycleRejection::StaleIdentity)?
                            .started = true;
                        Self::merge(
                            &mut transition,
                            self.observe(ResourceObservation::OwnedStarted {
                                resource: key,
                                process,
                            })?,
                        );
                    }
                    RegistryObservation::FamilySettled { .. } => {
                        self.records[at]
                            .as_mut()
                            .ok_or(LifecycleRejection::StaleIdentity)?
                            .family_settled = true;
                    }
                    RegistryObservation::Failed { error, .. } => {
                        Self::merge(&mut transition, self.failed(key, Box::new(error), resources)?);
                    }
                    RegistryObservation::RootExited { process, code } => {
                        let next =
                            self.managed_exit_observation(key, ticket.process(), process, code)?;
                        Self::merge(&mut transition, next);
                        if self.records[at].as_ref().is_some_and(|record| record.shutdown.is_none())
                        {
                            match self.begin_cleanup(key, None, resources) {
                                Ok(next) => Self::merge(&mut transition, next),
                                Err(cause) => self.records[at]
                                    .as_mut()
                                    .ok_or(LifecycleRejection::StaleIdentity)?
                                    .cleanup_failure
                                    .retain(cause),
                            }
                        }
                    }
                    RegistryObservation::RootObservationRetired { .. } => {}
                }
            }
        }
        let result = self.progress(at, batch, resources);
        match result {
            Ok(next) => Self::merge(&mut transition, next),
            Err(cause) => Self::merge(&mut transition, self.failed(key, cause, resources)?),
        }
        if self.records[at].as_ref().is_some_and(|r| r.shutdown.is_some()) {
            match self.cleanup(at, resources) {
                Ok(next) => Self::merge(&mut transition, next),
                Err(cause) => {
                    let record =
                        self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
                    record.cleanup_failure.retain(cause);
                }
            }
        }
        Ok(self.undelivered.take().unwrap_or(transition))
    }
    /// Adopt actual source/readiness results before selecting later preparation work.
    fn progress(
        &mut self,
        at: usize,
        batch: &CompletionBatch,
        resources: &mut BackendResources<'_>,
    ) -> Result<ResourceTransition> {
        let custody_ready = match &mut self.custody {
            Some(custody) => custody.advance(resources)?,
            None => self.custody_options.is_none(),
        };
        if custody_ready {
            self.claim_resolved(at, resources)?;
        }
        let record = self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
        if record.source_job {
            match record.source.take_inspection() {
                Ok(Some(source)) => {
                    record.source_job = false;
                    record.source_lease = Some(source);
                    tracing::info!(operation="backend_preparation_transition",resource=?record.key,stage="SourceTerminal",outcome="Success",
                        "Original backend preparation advanced");
                }
                Ok(None) => {}
                Err(cause) => {
                    record.source_job = false;
                    return Err(Box::new(cause));
                }
            }
        }
        if let Some(output) = &mut record.output
            && !record.output_observed
            && let Some(completion) = output.completion.try_take_completion()
        {
            record.output_observed = true;
            if record.shutdown.is_none() {
                return Err(Box::new(OutputTerminalFailure(completion)));
            }
            record.output_terminal = Some(completion);
        }
        if let Some(close) = &mut record.source_close
            && let Some(completion) = close.try_take_completion()
        {
            record.source_closed = matches!(completion.outcome, JobOutcome::Success(()));
            record.source_terminal = Some(completion);
            record.source_close = None;
        }
        let step = record.campaign.consume(batch, resources.client.now()?)?;
        if record.shutdown.is_none() {
            self.launch(at, resources)?;
            self.probe(at, resources)?;
        }
        let record = self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
        if step == CampaignStep::Ready
            && record.shutdown.is_none()
            && record.readiness.is_none()
            && let Some(proof) = record.campaign.take_readiness()
        {
            record.budget.probe_until(resources.client.now()?)?;
            proof.validate_current()?;
            if proof.managed_resource()
                != matches!(record.target, BackendTarget::Managed(_)).then_some(record.key)
            {
                return Err(Box::new(LifecycleRejection::StaleIdentity));
            }
            let port = match &record.target {
                BackendTarget::Managed(config) => config.port().get(),
                BackendTarget::Attached(origin) => origin.port().get(),
            };
            let origin = match &record.target {
                BackendTarget::Attached(origin) => *origin,
                BackendTarget::Managed(_) => {
                    LoopbackOrigin::parse(&format!("http://127.0.0.1:{port}"), port)?
                }
            };
            let context = BackendReadyMetadata::capture(record.key, origin, &proof)?;
            record.readiness = Some(proof);
            let key = record.key;
            let transition = self.observe(ResourceObservation::Ready { resource: key, origin })?;
            let record = self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
            record.ready_origin = Some(origin);
            record.ready_context = Some(context);
            return Ok(transition);
        }
        let cleanup_current = match record.shutdown {
            Some((_, budget)) => resources.client.now()? < budget.settle_until,
            None => true,
        };
        if step == CampaignStep::Progressed || step == CampaignStep::Failed {
            if step == CampaignStep::Failed
                && record.shutdown.is_none()
                && let Some(cause) = record.campaign.take_failure()
            {
                return Err(Box::new(cause));
            }
            if !record.campaign.settled() && cleanup_current {
                let until = record
                    .shutdown
                    .map_or(record.budget.until(), |(_, budget)| budget.settle_until);
                let spec = Self::spec(
                    resources,
                    Operation::Readiness,
                    JobClass::InfrastructureLifecycle,
                    sugarsubstitute_comfy::readiness_transport::READINESS_TRANSPORT_BYTES,
                    Some(until),
                )?;
                match record.campaign.submit_cleanup(resources.client, resources.cleanup, spec) {
                    Ok(_) => {}
                    Err(sugarsubstitute_comfy::readiness_campaign::CampaignError::Rejected(
                        sugarsubstitute_comfy::readiness_campaign::CampaignRejection::WrongPhase,
                    )) => {}
                    Err(cause) => return Err(Box::new(cause)),
                }
            }
        }
        Ok(ResourceTransition::default())
    }
    /// Retain the complete first failure in policy before diagnostics and cleanup.
    pub(super) fn failed(
        &mut self,
        key: ResourceKey,
        cause: Box<dyn Error + Send + Sync>,
        resources: &mut BackendResources<'_>,
    ) -> Result<ResourceTransition> {
        let at = self.index(key)?;
        let record = self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
        if record.failure_published {
            record.cleanup_failure.retain(cause);
            return Ok(ResourceTransition::default());
        }
        record.failure_published = true;
        let failure = LifecycleFailure::new(LifecycleFailureCode::Preparation, cause);
        let mut transition =
            self.observe(ResourceObservation::Failed { resource: key, failure })?;
        if let Some(failure) = self.failure(key) {
            let observation = FailureObservation::retained(failure);
            if observation.kind == FailureKind::Source {
                tracing::info!(operation="backend_preparation_transition",resource=?key,stage="SourceTerminal",outcome="Error",
                    "Original backend preparation advanced");
            }
            tracing::warn!(operation="backend_preparation_failure",resource=?key,kind=?observation.kind,stage=observation.stage,terminal=?observation.terminal,exit_code=observation.exit_code,
                native_operation=observation.native_operation,native_code=observation.native_code,
                "Original backend preparation failure retained");
        }
        match self.begin_cleanup(key, None, resources) {
            Ok(next) => Self::merge(&mut transition, next),
            Err(cause) => self.records[at]
                .as_mut()
                .ok_or(LifecycleRejection::StaleIdentity)?
                .cleanup_failure
                .retain(cause),
        }
        Ok(transition)
    }
}
