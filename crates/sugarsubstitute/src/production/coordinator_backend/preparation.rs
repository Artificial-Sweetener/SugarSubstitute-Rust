// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt finite source/claim/output preparations before exposing native progress.
use super::*;
impl CoordinatorBackend {
    pub(super) fn effects(
        &mut self,
        transition: &mut ResourceTransition,
        input: &mut Option<BackendPreparation>,
        original_shutdown: Option<ShutdownBudget>,
        resources: &mut BackendResources<'_>,
    ) -> Result<()> {
        let effects = transition.effects.clone();
        for effect in effects {
            match effect {
                ResourceEffect::Prepare { resource, target } => {
                    if let Err(cause) = self.prepare(resource, target, input, resources) {
                        if self.index(resource).is_ok() {
                            let next = self.failed(resource, cause, resources)?;
                            Self::merge(transition, next);
                        } else {
                            // No input/native owner was admitted: caller still owns the input.
                            let failure =
                                LifecycleFailure::new(LifecycleFailureCode::Preparation, cause);
                            Self::merge(
                                transition,
                                self.observe(ResourceObservation::Failed { resource, failure })?,
                            );
                            Self::merge(
                                transition,
                                self.observe(ResourceObservation::Settled(resource))?,
                            );
                        }
                    }
                }
                ResourceEffect::StopOwned { resource }
                | ResourceEffect::DisconnectAttached { resource } => {
                    self.begin_cleanup(resource, original_shutdown, resources)?;
                }
            }
        }
        Ok(())
    }
    fn prepare(
        &mut self,
        key: ResourceKey,
        target: BackendTarget,
        input: &mut Option<BackendPreparation>,
        resources: &mut BackendResources<'_>,
    ) -> Result<()> {
        let at = self
            .records
            .iter()
            .position(Option::is_none)
            .or_else(|| self.records.iter().position(|r| r.as_ref().is_some_and(|r| r.settled)))
            .ok_or(LifecycleRejection::Overloaded)?;
        let original = input.as_ref().ok_or(LifecycleRejection::WrongPhase)?;
        if matches!(target, BackendTarget::Managed(_)) != original.environment.is_some() {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        let kind = match target {
            BackendTarget::Managed(_) => ConnectionBudgetKind::Managed,
            BackendTarget::Attached(_) => ConnectionBudgetKind::Attached,
        };
        let budget = ConnectionBudget::new(kind, resources.client.now()?)?;
        let campaign = ReadinessCampaign::new(target.clone(), budget)?;
        let scope = resources.client.create_scope(
            ScopeSpec { id: resources.scopes.allocate()?, deadline: Some(budget.until()) },
            Some(resources.native),
        )?;
        let input = input.take().ok_or(LifecycleRejection::WrongPhase)?;
        self.records[at] = Some(Backend {
            key,
            target,
            budget,
            resolved: None,
            ready_origin: None,
            scope,
            source: InstalledFrontendOwner::new(input.source),
            source_job: false,
            source_lease: None,
            source_close: None,
            source_terminal: None,
            source_closed: true,
            environment: input.environment,
            resolution: None,
            resolution_pending: false,
            resolution_retiring: false,
            claim_pending: false,
            backend_admitted: false,
            claimed: false,
            release_pending: false,
            released: false,
            launch: None,
            started: false,
            family_settled: false,
            output: None,
            output_terminal: None,
            output_observed: false,
            campaign,
            readiness: None,
            shutdown: None,
            failure_published: false,
            settled: false,
            cleanup_failure: None,
        });
        let spec = Self::spec(
            resources,
            Operation::Readiness,
            JobClass::InfrastructureLifecycle,
            FRONTEND_SOURCE_BYTES,
            Some(budget.until()),
        )?;
        let record = self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
        record.source.submit_inspection(resources.client, &record.scope, spec)?;
        record.source_job = true;
        record.source_closed = false;
        if let BackendTarget::Managed(configuration) = &record.target {
            let spec = Self::spec(
                resources,
                Operation::NativeLifecycle,
                JobClass::InfrastructureLifecycle,
                CLAIM_OPERATION_BYTES,
                Some(budget.until()),
            )?;
            record.resolution = Some(resources.authority.claims_mut().submit_resolve_launch(
                resources.client,
                &record.scope,
                spec,
                configuration.clone(),
            )?);
            record.resolution_pending = true;
        }
        Ok(())
    }
    pub(super) fn launch(&mut self, at: usize, resources: &mut BackendResources<'_>) -> Result<()> {
        let record = self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
        if !record.claimed || record.launch.is_some() || record.shutdown.is_some() {
            return Ok(());
        }
        record.budget.probe_until(resources.client.now()?)?;
        if record.output.is_none() {
            let spec = Self::spec(
                resources,
                Operation::NativeLifecycle,
                JobClass::NativeObservation,
                MIN_WINDOWS_OUTPUT_BYTES,
                None,
            )?;
            record.output = Some(resources.client.reserve_windows_output(
                resources.native,
                spec,
                record.key,
            )?);
        }
        let output = record.output.as_ref().ok_or(LifecycleRejection::WrongPhase)?;
        if !output.control.snapshot().prepared {
            return Ok(());
        }
        let stdio = output.control.take_child_stdio()?;
        let environment = record.environment.take().ok_or(LifecycleRejection::WrongPhase)?;
        let input =
            resources.authority.claims_mut().prepare_managed_launch(record.key, environment)?;
        let input = match input.with_owned_output(&output.control, stdio) {
            Ok(input) => input,
            Err(rejected) => {
                drop(rejected.input);
                drop(rejected.stdio);
                return Err(Box::new(rejected.error));
            }
        };
        let root_watch = Self::spec(
            resources,
            Operation::NativeLifecycle,
            JobClass::NativeObservation,
            4096,
            None,
        )?;
        let creation = Self::spec(
            resources,
            Operation::NativeLifecycle,
            JobClass::InfrastructureLifecycle,
            input.required_reservation_bytes(),
            Some(record.budget.until()),
        )?;
        let native_scope = ScopeSpec { id: resources.scopes.allocate()?, deadline: None };
        let (claims, registry) =
            resources.authority.owners_mut().ok_or(LifecycleRejection::WrongPhase)?;
        record.launch = Some(claims.submit_managed_launch(
            resources.client,
            registry,
            &record.scope,
            ClaimLaunchJobs { native_scope, creation, root_watch },
            input,
        )?);
        Ok(())
    }
    pub(super) fn probe(&mut self, at: usize, resources: &mut BackendResources<'_>) -> Result<()> {
        let record = self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
        if record.shutdown.is_some()
            || record.source_lease.is_none()
            || (matches!(record.target, BackendTarget::Managed(_)) && !record.started)
        {
            return Ok(());
        }
        let now = resources.client.now()?;
        if !record.campaign.needs_input(now)? {
            return Ok(());
        }
        let source = record.source_lease.as_ref().ok_or(LifecycleRejection::WrongPhase)?.clone();
        let input = match &record.target {
            BackendTarget::Managed(_) => {
                let configuration =
                    record.resolved.as_ref().ok_or(LifecycleRejection::WrongPhase)?;
                let registry =
                    resources.authority.registry().ok_or(LifecycleRejection::WrongPhase)?;
                let ticket = record.launch.ok_or(LifecycleRejection::WrongPhase)?;
                ReadinessProbeInput::managed(
                    source,
                    registry.managed_listener_lease(ticket, record.key)?,
                    configuration.clone(),
                )?
            }
            BackendTarget::Attached(origin) => {
                ReadinessProbeInput::attached(source, origin.port().get())?
            }
        };
        let spec = Self::spec(
            resources,
            Operation::Readiness,
            JobClass::InfrastructureLifecycle,
            sugarsubstitute_comfy::readiness_transport::READINESS_TRANSPORT_BYTES,
            Some(record.budget.probe_until(now)?),
        )?;
        record.campaign.submit_attempt(resources.client, &record.scope, spec, input)?;
        Ok(())
    }
}
