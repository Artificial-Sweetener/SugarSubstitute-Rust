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
    ) -> Result<()> {
        let at = self.index(key)?;
        let record = self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
        if record.shutdown.is_some() {
            return Ok(());
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
        record.readiness = None;
        record.campaign.cancel(budget);
        record.scope.cancel();
        if let Some(output) = &record.output {
            output.control.close();
        }
        if let Some(ticket) = record.launch {
            resources.authority.registry_mut().ok_or(LifecycleRejection::WrongPhase)?.begin_stop(
                ticket.process(),
                shutdown,
                budget,
            )?;
        }
        Ok(())
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
                FRONTEND_SOURCE_BYTES,
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
            if !snapshot.settled && !snapshot.creation_pending && !snapshot.cleanup_pending {
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
        let output_settled = record
            .output
            .as_ref()
            .is_none_or(|output| output.control.snapshot().retired && record.output_observed);
        if record.source_closed
            && record.campaign.settled()
            && output_settled
            && record.resolution.is_none()
            && !record.resolution_pending
            && !record.claim_pending
            && (!record.backend_admitted || record.released)
            && (record.launch.is_none() || record.family_settled)
        {
            if let Some(ticket) = record.launch {
                resources
                    .authority
                    .registry_mut()
                    .ok_or(LifecycleRejection::WrongPhase)?
                    .retire_settled(ticket.process())?;
                record.launch = None;
            }
            record.settled = true;
            let key = record.key;
            return self.observe(ResourceObservation::Settled(key));
        }
        Ok(ResourceTransition::default())
    }
}
