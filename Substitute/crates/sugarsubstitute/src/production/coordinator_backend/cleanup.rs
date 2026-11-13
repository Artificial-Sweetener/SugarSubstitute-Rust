// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Require checked source, probe, output, family and claim retirement independently.
use super::*;
impl CoordinatorBackend {
    pub(super) fn begin_cleanup(
        &mut self,
        key: ResourceKey,
        original: Option<ShutdownBudget>,
        resources: &mut BackendResources<'_>,
    ) -> Result<ResourceTransition> {
        let at = self.index(key)?;
        let record = self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
        if record.shutdown.is_some() {
            return Ok(ResourceTransition::default());
        }
        let next =
            self.next_shutdown.checked_add(1).ok_or(LifecycleRejection::IdentityExhausted)?;
        let shutdown = ShutdownId::new(next)?;
        let budget = match original {
            Some(budget) => budget,
            None => ShutdownBudget::new(resources.client.now()?)?,
        };
        self.next_shutdown = next;
        record.shutdown = Some((shutdown, budget));
        self.output_clients.retire_resource(key);
        record.output_fanout.current = None;
        record.readiness = None;
        record.campaign.cancel(budget);
        record.scope.cancel();
        if let Some(output) = &record.output {
            output.control.request_backend_shutdown();
        }
        if let Some(ticket) = record.launch
            && let Err(cause) = resources
                .authority
                .registry_mut()
                .ok_or(LifecycleRejection::WrongPhase)?
                .begin_stop(ticket.process(), shutdown, budget)
        {
            record.cleanup_failure.retain(Box::new(cause));
        }
        // A late authentic request may arrive after grace. A past wake is not
        // an event, so admit its actual due finite step before returning to the pump.
        if resources.client.now()? >= budget.graceful_until {
            match self.cleanup(at, resources) {
                Ok(next) => Ok(next),
                Err(cause) => {
                    self.records[at]
                        .as_mut()
                        .ok_or(LifecycleRejection::StaleIdentity)?
                        .cleanup_failure
                        .retain(cause);
                    Ok(ResourceTransition::default())
                }
            }
        } else {
            Ok(ResourceTransition::default())
        }
    }
    pub(super) fn cleanup(
        &mut self,
        at: usize,
        resources: &mut BackendResources<'_>,
    ) -> Result<ResourceTransition> {
        let record = self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
        if record.settled {
            return Ok(ResourceTransition::default());
        }
        let (_, budget) = record.shutdown.ok_or(LifecycleRejection::WrongPhase)?;
        let now = resources.client.now()?;
        if now >= budget.settle_until && record.cleanup_failure.first().is_none() {
            tracing::warn!(operation="backend_attempt_cleanup",resource=?record.key,
                source_pending=record.source_job || record.source_close.is_some(),
                source_closed=record.source_closed,probe_settled=record.campaign.settled(),
                resolution_pending=record.resolution_pending || record.resolution_retiring,
                claim_pending=record.claim_pending || record.release_pending,
                family_settled=record.family_settled,
                "Original backend cleanup cutoff reached with retained owner state");
        }
        record.cleanup_failure.observe(now, budget);
        if self.pack.gates(record.key) {
            self.pack.cancel();
        }
        if now >= budget.settle_until {
            return self.finish_cleanup(at, resources);
        }
        Self::close_campaign(record, resources, now)?;
        if !record.source_job
            && !record.source_closed
            && record.source_close.is_none()
            && record.source_terminal.is_none()
        {
            record.source_lease = None;
            let spec = Self::spec(
                resources,
                Operation::Readiness,
                JobClass::InfrastructureLifecycle,
                FRONTEND_SOURCE_CLOSE_BYTES,
                Some(budget.settle_until),
            )?;
            record.source_close =
                Some(record.source.submit_close(resources.client, resources.cleanup, spec)?);
        }
        if !record.resolution_pending
            && !record.resolution_retiring
            && let Some(ticket) = &record.resolution
        {
            let spec = Self::spec(
                resources,
                Operation::NativeLifecycle,
                JobClass::InfrastructureLifecycle,
                CLAIM_OPERATION_BYTES,
                Some(budget.settle_until),
            )?;
            resources.authority.claims_mut().submit_retire_resolution(
                resources.client,
                resources.cleanup,
                spec,
                ticket,
            )?;
            record.resolution_retiring = true;
        }
        if let Some(ticket) = record.launch {
            let snapshot = resources
                .authority
                .registry()
                .ok_or(LifecycleRejection::WrongPhase)?
                .snapshot(ticket.process())?;
            if snapshot.cleanup_step_due(resources.client.now()?) {
                let watch = if snapshot.member_watch_required {
                    Some(Self::spec(
                        resources,
                        Operation::NativeLifecycle,
                        JobClass::NativeObservation,
                        4096,
                        None,
                    )?)
                } else {
                    None
                };
                let spec = Self::spec(
                    resources,
                    Operation::NativeLifecycle,
                    JobClass::InfrastructureLifecycle,
                    4096,
                    Some(budget.settle_until),
                )?;
                resources
                    .authority
                    .registry_mut()
                    .ok_or(LifecycleRejection::WrongPhase)?
                    .submit_cleanup_step(
                        resources.client,
                        ticket.process(),
                        resources.cleanup,
                        spec,
                        watch,
                    )?;
            }
        }
        if record.backend_admitted
            && !record.claim_pending
            && !record.released
            && !record.release_pending
            && (record.launch.is_none() || record.family_settled)
        {
            let spec = Self::spec(
                resources,
                Operation::NativeLifecycle,
                JobClass::InfrastructureLifecycle,
                CLAIM_OPERATION_BYTES,
                Some(budget.settle_until),
            )?;
            let (claims, registry) =
                resources.authority.owners_mut().ok_or(LifecycleRejection::WrongPhase)?;
            claims.submit_backend_release(
                resources.client,
                resources.cleanup,
                spec,
                registry,
                record.key,
            )?;
            record.release_pending = true;
        }
        self.finish_cleanup(at, resources)
    }
    /// Retire an admitted probe independently of the custodian's admission health.
    /// A dead helper cannot veto cleanup of the existing HTTP transport.
    pub(super) fn close_campaign(
        record: &mut Backend,
        resources: &BackendResources<'_>,
        now: Moment,
    ) -> Result<()> {
        if record.campaign.settled() {
            return Ok(());
        }
        let until = record.campaign.cleanup_deadline(now)?;
        let spec = Self::spec(
            resources,
            Operation::Readiness,
            JobClass::InfrastructureLifecycle,
            sugarsubstitute_comfy::readiness_transport::READINESS_TRANSPORT_BYTES,
            Some(until),
        )?;
        match record.campaign.submit_cleanup(resources.client, resources.cleanup, spec) {
            Ok(_) => Ok(()),
            Err(sugarsubstitute_comfy::readiness_campaign::CampaignError::Rejected(
                sugarsubstitute_comfy::readiness_campaign::CampaignRejection::WrongPhase,
            )) => Ok(()),
            Err(cause) => Err(Box::new(cause)),
        }
    }
    /// Observe actual late closure without admitting another native effect or renewing time.
    fn finish_cleanup(
        &mut self,
        at: usize,
        resources: &mut BackendResources<'_>,
    ) -> Result<ResourceTransition> {
        let record = self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
        if (record.launch.is_none() || record.family_settled)
            && let Some(output) = &record.output
        {
            output.control.close();
        }
        let output_settled = record.output.as_ref().is_none_or(|output| {
            let actual = output.control.snapshot();
            actual.identity
                == sugarsubstitute_execution::windows_output::OutputIdentity::Managed(record.key)
                && actual.retired
                && record.output_observed
                && output.control.loan_available()
                && record.output_fanout.current.is_none()
        });
        if record.source_closed
            && !self.pack.gates(record.key)
            && record.campaign.settled()
            && output_settled
            && record.resolution.is_none()
            && !record.resolution_pending
            && !record.claim_pending
            && (!record.backend_admitted || record.released)
            && (record.launch.is_none() || record.family_settled)
        {
            if let Some(custody) = &self.custody
                && let Some(ticket) = record.launch
            {
                if !custody.release(record.key, budget_of(record)?, resources)? {
                    return Ok(ResourceTransition::default());
                }
                let registry =
                    resources.authority.registry_mut().ok_or(LifecycleRejection::WrongPhase)?;
                registry.observe_backend_custody_release(custody.parent()?, ticket)?;
                if registry.custody_retirement_pending(ticket.process())? {
                    if !registry.snapshot(ticket.process())?.cleanup_step_pending
                        && resources.client.now()? < budget_of(record)?.settle_until
                    {
                        let spec = Self::spec(
                            resources,
                            Operation::NativeLifecycle,
                            JobClass::InfrastructureLifecycle,
                            4096,
                            Some(budget_of(record)?.settle_until),
                        )?;
                        resources
                            .authority
                            .registry_mut()
                            .ok_or(LifecycleRejection::WrongPhase)?
                            .submit_cleanup_step(
                            resources.client,
                            ticket.process(),
                            resources.cleanup,
                            spec,
                            None,
                        )?;
                    }
                    return Ok(ResourceTransition::default());
                }
            }
            if let Some(ticket) = record.launch {
                resources
                    .authority
                    .registry_mut()
                    .ok_or(LifecycleRejection::WrongPhase)?
                    .retire_settled(ticket.process())?;
                record.launch = None;
            }
            // No native reader, launch pin or fanout guard remains. Historical failures
            // retain their original completion credit; successful output custody is obsolete.
            record.output = None;
            let normal_terminal =
                record.output_terminal.as_ref().is_some_and(|terminal| match &terminal.outcome {
                    JobOutcome::Success(report) => report.retired
                        && report.identity
                            == sugarsubstitute_execution::windows_output::OutputIdentity::Managed(
                                record.key,
                            ),
                    JobOutcome::Cancelled => record.shutdown.is_some(),
                    _ => false,
                });
            if normal_terminal {
                record.output_terminal = None;
            }
            record.settled = true;
            let key = record.key;
            return self.observe(ResourceObservation::Settled(key));
        }
        Ok(ResourceTransition::default())
    }
}
fn budget_of(record: &Backend) -> Result<ShutdownBudget> {
    Ok(record.shutdown.ok_or(LifecycleRejection::WrongPhase)?.1)
}
