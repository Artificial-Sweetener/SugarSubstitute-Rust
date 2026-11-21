// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route actual adapter observations without deriving readiness from terminal metadata.
use super::*;
impl CoordinatorBackend {
    /// Keep profile custody supervised whenever no resource has an active progress turn.
    /// Historical settled records cannot keep this independent owner from advancing.
    pub(crate) fn maintain_profile_custody_without_resources(
        &mut self,
        resources: &mut BackendResources<'_>,
    ) -> Result<()> {
        if self.resource_keys().iter().all(Option::is_none)
            && let Some(custody) = &mut self.custody
        {
            custody.advance(resources)?;
        }
        Ok(())
    }
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
                    record.backend_binding =
                        Some(resources.authority.claims().resolved_backend_binding(ticket)?);
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
                transition = self.claim_work_ended(at, &target, Box::new(cause), resources)?;
            }
            ClaimObservation::Stopped { target, kind } => {
                transition =
                    self.claim_work_ended(at, &target, Box::new(Stopped(kind)), resources)?;
            }
            other => return Ok((transition, Some(other))),
        }
        Ok((self.undelivered.take().unwrap_or(transition), None))
    }

    /// Take the end of a claim or resolution job that produced no result. Preparation work the
    /// record's own cleanup cancelled is noted; anything else is this record's failure.
    pub(super) fn claim_work_ended(
        &mut self,
        at: usize,
        target: &ClaimTarget,
        cause: Box<dyn Error + Send + Sync>,
        resources: &mut BackendResources<'_>,
    ) -> Result<ResourceTransition> {
        let record = self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
        let key = record.key;
        if Self::end_claim_work(record, target) {
            tracing::info!(operation="backend_preparation_cancelled",resource=?key,cause=%cause,
                "Backend preparation ended by its own cleanup");
            return Ok(ResourceTransition::default());
        }
        self.failed(key, cause, resources)
    }

    /// Record that a claim or resolution job ended without its result, and say whether it was
    /// preparation work this record's own cleanup had cancelled.
    ///
    /// Cleanup cancels the record's scope, so a claim or resolution still being prepared ends
    /// stopped or failed. That is the cancellation taking effect, not a failed cleanup step:
    /// the release and retirement cleanup submits afterwards report on their own.
    fn end_claim_work(record: &mut Backend, target: &ClaimTarget) -> bool {
        let key = record.key;
        let preparing = match target {
            ClaimTarget::Resolution(_) => {
                tracing::info!(operation="backend_preparation_transition",resource=?key,stage="ResolutionTerminal",
                    "Original backend preparation advanced");
                std::mem::take(&mut record.resolution_pending)
            }
            ClaimTarget::Backend(_) => {
                if record.claim_pending {
                    tracing::info!(operation="backend_preparation_transition",resource=?key,stage="ClaimTerminal",
                        "Original backend preparation advanced");
                }
                record.release_pending = false;
                std::mem::take(&mut record.claim_pending)
            }
            ClaimTarget::Coordinator => false,
        };
        preparing && record.shutdown.is_some()
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
                        Self::merge(
                            &mut transition,
                            self.managed_started_observation(
                                key,
                                ticket.process(),
                                process,
                                resources.client.now()?,
                            )?,
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
                        if self.records[at].as_ref().is_some_and(ended_before_ready) {
                            self.pack.backend_stopped(resources)?;
                        }
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
        self.advance_pack(resources)?;
        let custody = match &mut self.custody {
            Some(custody) => custody.advance(resources),
            None => Ok(self.custody_options.is_none()),
        };
        let (custody_ready, step) = self.consume_existing(at, batch, resources, custody)?;
        if custody_ready {
            self.claim_resolved(at, resources)?;
        }
        let record = self.records[at].as_ref().ok_or(LifecycleRejection::StaleIdentity)?;
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
            record.campaign.probe_until(resources.client.now()?)?;
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
            if matches!(record.target, BackendTarget::Managed(_)) {
                // Readiness does not see node registration; the page's session decides.
                self.pack.observe(
                    sugarsubstitute_supervision::pack_install::TrialEvent::BackendStartUnobservable,
                    resources,
                )?;
            }
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
                Self::close_campaign(record, resources, resources.client.now()?)?;
            }
        }
        Ok(ResourceTransition::default())
    }
    /// Adopt already-owned results even when the fresh custody admission failed.
    /// Later observation causes use the record's existing bounded cleanup slots.
    fn consume_existing(
        &mut self,
        at: usize,
        batch: &CompletionBatch,
        resources: &BackendResources<'_>,
        custody: Result<bool>,
    ) -> Result<(bool, CampaignStep)> {
        let (custody_ready, mut failure) = match custody {
            Ok(ready) => (ready, None),
            Err(cause) => (false, Some(cause)),
        };
        let record = self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
        if let Err(cause) = consume_source(record, resources.client.now()?) {
            if failure.is_none() {
                failure = Some(cause);
            } else {
                record.cleanup_failure.retain(cause);
            }
        }
        if let Some(output) = &mut record.output
            && !record.output_observed
            && let Some(completion) = output.completion.try_take_completion()
        {
            record.output_observed = completion.scope_id == output.completion.scope_id()
                && completion.spec == output.completion.spec();
            // EOF can precede the registered process-exit observation. It closes
            // output only; the registry owns crash classification and recovery.
            if !record.output_observed {
                record.output_terminal = Some(completion);
                failure.get_or_insert_with(|| Box::new(LifecycleRejection::StaleIdentity));
            } else if record.shutdown.is_none()
                && !matches!(completion.outcome, JobOutcome::Success(_))
            {
                let cause = Box::new(OutputTerminalFailure(completion));
                if failure.is_none() {
                    failure = Some(cause);
                } else {
                    record.cleanup_failure.retain(cause);
                }
            } else {
                record.output_terminal = Some(completion);
            }
        }
        let step = resources
            .client
            .now()
            .map_err(|cause| Box::new(cause) as Box<dyn Error + Send + Sync>)
            .and_then(|now| {
                record
                    .campaign
                    .consume(batch, now)
                    .map_err(|cause| Box::new(cause) as Box<dyn Error + Send + Sync>)
            });
        match step {
            Ok(step) => match failure {
                Some(cause) => Err(cause),
                None => Ok((custody_ready, step)),
            },
            Err(cause) => match failure {
                Some(first) => {
                    record.cleanup_failure.retain(cause);
                    Err(first)
                }
                None => Err(cause),
            },
        }
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
        // A managed backend that fails before it is ready did not come up with the SugarCubes
        // version being judged, exactly as if its process had ended: that version is restored
        // before the next attempt starts the backend. A ComfyUI that is listening but never
        // serves SugarCubes' extension (the new version failed to import) ends here.
        let stopped =
            matches!(record.target, BackendTarget::Managed(_)) && ended_before_ready(record);
        if stopped {
            self.pack.backend_stopped(resources)?;
        }
        let failure =
            LifecycleFailure::new(super::failure::preparation_code(cause.as_ref()), cause);
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

/// The backend ended or failed before it was ready, outside an intended shutdown.
fn ended_before_ready(record: &Backend) -> bool {
    record.shutdown.is_none() && record.readiness.is_none()
}

/// Consume only this generation's actual source jobs; terminal metadata never proves file closure.
fn consume_source(record: &mut Backend, now: Moment) -> Result<()> {
    if record.source_job {
        match record.source.take_inspection() {
            Ok(Some(source)) => {
                record.source_job = false;
                if record.shutdown.is_none() && matches!(record.target, BackendTarget::Attached(_))
                {
                    record.campaign.admit_readiness_wait(&record.scope, now)?;
                }
                record.source_lease = Some(source);
                tracing::info!(operation="backend_preparation_transition",resource=?record.key,stage="SourceTerminal",outcome="Success",
                    "Original backend preparation advanced");
            }
            Ok(None) => {}
            Err(cause) => {
                record.source_job = false;
                if record.shutdown.is_some()
                    && matches!(
                        cause.terminal_kind(),
                        Some(TerminalKind::Cancelled | TerminalKind::TimedOut)
                    )
                {
                    tracing::info!(operation="backend_preparation_transition",resource=?record.key,
                        stage="SourceTerminal",outcome=?cause.terminal_kind(),
                        "Original source inspection stopped during existing cleanup");
                } else {
                    return Err(Box::new(cause));
                }
            }
        }
    }
    if let Some(close) = &mut record.source_close
        && let Some(completion) = close.try_take_completion()
    {
        let original = completion.scope_id == close.scope_id() && completion.spec == close.spec();
        record.source_closed = original && matches!(completion.outcome, JobOutcome::Success(()));
        if !record.source_closed {
            record.source_terminal = Some(completion);
        }
        record.source_close = None;
        if !original {
            return Err(LifecycleRejection::StaleIdentity.into());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "source_terminal_tests.rs"]
mod source_terminal_tests;
