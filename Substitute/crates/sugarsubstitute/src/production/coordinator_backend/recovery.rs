// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual managed death, independent cleanup and exact explicit retry lineage.
use super::*;
use sugarsubstitute_application::supervision::InvalidatedLeaseState;

/// Closed recoverable classifications; neither variant authorizes native effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecoverableBackendFailure {
    /// The same original retained registry watch observed this unexpected exit.
    ManagedExited { process: ProcessKey, status: u32 },
    /// An explicitly admitted successor failed while its original outage remains owned.
    RetryFailed,
}

impl CoordinatorBackend {
    /// Consume only the matching retained registry ContainedStarted observation.
    /// This admits a live wait, never Ready, listener ownership or native settlement.
    pub(super) fn managed_started_observation(
        &mut self,
        key: ResourceKey,
        expected: ProcessKey,
        observed: ProcessKey,
        now: Moment,
    ) -> Result<ResourceTransition> {
        if expected != observed || expected.owner != key.owner {
            return Err(Box::new(LifecycleRejection::StaleIdentity));
        }
        let at = self.index(key)?;
        let record = self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
        if !matches!(record.target, BackendTarget::Managed(_)) {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        if record.shutdown.is_none() {
            record.campaign.admit_readiness_wait(&record.scope, now)?;
        }
        record.started = true;
        self.observe(ResourceObservation::OwnedStarted { resource: key, process: observed })
    }
    /// Route the exact retained launch's original native RootExited observation.
    /// Expected cleanup exits produce no outage. A root exit never asserts family,
    /// listener, source, output or claim settlement; their original owners still run.
    pub(super) fn managed_exit_observation(
        &mut self,
        key: ResourceKey,
        expected: ProcessKey,
        observed: ProcessKey,
        status: u32,
    ) -> Result<ResourceTransition> {
        if expected != observed || expected.owner != key.owner {
            return Err(Box::new(LifecycleRejection::StaleIdentity));
        }
        let at = self.index(key)?;
        let record = self.records[at].as_ref().ok_or(LifecycleRejection::StaleIdentity)?;
        if !matches!(record.target, BackendTarget::Managed(_)) {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        if record.shutdown.is_some() {
            return Ok(ResourceTransition::default());
        }
        tracing::warn!(operation="managed_backend_root_exited",resource=?key,process=?observed,status,
            "Actual retained managed backend root exit observed; independent cleanup remains required");
        let failure = LifecycleFailure::new(
            LifecycleFailureCode::UnexpectedExit,
            Box::new(RootExit { process: observed, status }),
        );
        let transition = self.observe(ResourceObservation::ManagedExited {
            resource: key,
            process: observed,
            status,
            failure,
        })?;
        self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?.failure_published =
            true;
        self.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?.managed_exit =
            Some((observed, status));
        Ok(transition)
    }

    /// Distinguish only an actual retained managed outage from unrelated fatal errors.
    /// No source/claim/output/native cleanup failure is exempted. The caller retains
    /// the original failure and keeps progressing cleanup; `Some` never clears it.
    pub(crate) fn recoverable_failure(
        &self,
        key: ResourceKey,
    ) -> Result<Option<RecoverableBackendFailure>> {
        let Some(failure) = self.policy.failure(key) else {
            return Ok(None);
        };
        if !self.policy.retains_outage(key) || self.cleanup_failure(key).is_some() {
            return Ok(None);
        }
        // Failed admission before record installation has no native owner to retire.
        // Its original same-resource source must still be retained independently.
        let record = self.retained_source(key)?;
        if !matches!(record.target, BackendTarget::Managed(_)) {
            return Ok(None);
        }
        if let Some(exit) = failure.source().and_then(|cause| cause.downcast_ref::<RootExit>()) {
            if failure.code() != LifecycleFailureCode::UnexpectedExit {
                return Ok(None);
            }
            return Ok(Some(RecoverableBackendFailure::ManagedExited {
                process: exit.process,
                status: exit.status,
            }));
        }
        Ok(Some(RecoverableBackendFailure::RetryFailed))
    }

    /// Read exact authenticated policy state; native settlement and cause transfer remain independent.
    pub(crate) fn invalidated_lease(
        &self,
        binding: ClientBinding,
        failed: LeaseKey,
    ) -> Result<InvalidatedLeaseState> {
        Ok(self.policy.invalidated_lease(binding, failed)?)
    }
    /// Return the bounded original distinct-run cohort; presentation cannot enlarge it.
    pub(crate) fn affected_clients(&self, binding: ClientBinding, failed: LeaseKey) -> Result<u16> {
        Ok(self.policy.affected_clients(binding, failed)?)
    }
    /// Acknowledge the exact live ready pair only after genuine native child adoption.
    /// A queued or written replacement response does not authorize calling this method.
    pub(crate) fn adopt_replacement(
        &mut self,
        binding: ClientBinding,
        failed: LeaseKey,
        current: LeaseKey,
    ) -> Result<()> {
        self.policy.adopt_replacement(binding, failed, current)?;
        Ok(())
    }

    /// Return only the exact current failure generation in this retained outage lineage.
    /// This is matching data for cause transfer, not native retirement authority.
    pub(crate) fn recovery_cause_key(
        &self,
        binding: ClientBinding,
        failed: LeaseKey,
    ) -> Result<ResourceKey> {
        self.policy.invalidated_lease(binding, failed)?;
        Ok(self.policy.replacement_lease(binding, failed)?.unwrap_or(failed).resource)
    }

    /// Move the one original policy cause into a caller's already-funded empty slot.
    /// `false` retains the cause while actual cleanup remains unresolved. An occupied
    /// slot rejects before mutation. Independent cleanup failures stay fatal and keep
    /// retry unavailable; no clone, incident acknowledgement or native authority is inferred.
    pub(crate) fn transfer_recovery_failure(
        &mut self,
        key: ResourceKey,
        destination: &mut Option<LifecycleFailure>,
    ) -> Result<bool> {
        if destination.is_some() {
            return Err(Box::new(LifecycleRejection::Overloaded));
        }
        if !self.policy.retains_outage(key) {
            return Err(Box::new(LifecycleRejection::StaleIdentity));
        }
        if !self.attempt_settled(key) {
            return Ok(false);
        }
        if self.cleanup_failure(key).is_some() {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        let Some(failure) = self.policy.take_failure(key)? else {
            return Ok(false);
        };
        *destination = Some(failure);
        Ok(true)
    }

    /// Validate the exact original Parent selection and current failed native generation
    /// before policy retry mutation. The existing Parent input remains borrowed; no
    /// secret environment snapshot or new target is reconstructed by this owner.
    pub(super) fn retry_preparation(
        &self,
        binding: ClientBinding,
        failed: LeaseKey,
        input: Option<&BackendPreparation>,
    ) -> Result<ResourceKey> {
        if self.policy.invalidated_lease(binding, failed)? != InvalidatedLeaseState::RetryReady {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        let current = self.policy.replacement_lease(binding, failed)?.unwrap_or(failed).resource;
        let record = self.retained_source(current)?;
        let input = input.ok_or(LifecycleRejection::WrongPhase)?;
        if !self.attempt_settled(current)
            || self.cleanup_failure(current).is_some()
            || !matches!(record.target, BackendTarget::Managed(_))
            || input.environment.is_none()
            || !record.source.matches_selection(&input.source)
        {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        Ok(record.key)
    }

    /// Preserve the last genuinely admitted source when a later same-resource retry
    /// fails before creating its own native record. Policy settlement is checked by
    /// the caller; matching data never asserts source liveness or native retirement.
    pub(super) fn retained_source(&self, current: ResourceKey) -> Result<&Backend> {
        self.records
            .iter()
            .flatten()
            .filter(|record| {
                record.key.owner == current.owner
                    && record.key.id == current.id
                    && record.key.generation <= current.generation
            })
            .max_by_key(|record| record.key.generation)
            .ok_or_else(|| {
                Box::new(LifecycleRejection::StaleIdentity) as Box<dyn Error + Send + Sync>
            })
    }

    /// Borrow a live replacement's genuine Ready-produced metadata before
    /// accepting its exact App A-to-B transition. The recipient must retain it
    /// before delivery is acknowledged or another retry can recycle the record.
    /// A dead successor cannot pass this current-ready query.
    pub(crate) fn replacement_context(
        &self,
        binding: ClientBinding,
        previous: LeaseKey,
        current: LeaseKey,
    ) -> Result<Option<BackendReplacementContext<'_>>> {
        if self.policy.replacement_lease(binding, previous)? != Some(current)
            || current.run != binding.run
            || current.connection != binding.connection
        {
            return Err(Box::new(LifecycleRejection::StaleIdentity));
        }
        if self.policy.resource_phase(current.resource)
            != Some(sugarsubstitute_application::supervision::ResourcePhase::Ready)
        {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        let record = self.record(current.resource)?;
        let Some(context) = &record.ready_context else {
            return Ok(None);
        };
        if context.resource() != current.resource {
            return Err(Box::new(LifecycleRejection::StaleIdentity));
        }
        record.readiness.as_ref().ok_or(LifecycleRejection::WrongPhase)?.validate_current()?;
        Ok(Some(BackendReplacementContext { context, source: &record.source }))
    }

    /// Borrow actual dead-successor history for the original installed selection.
    /// This validates B independently because genuine B death removes App's old
    /// A notice. The Parent must separately retain the actual emitted A-to-B pair,
    /// native binding and accepted B notice, and establish old-A absence before
    /// using this binding-only context. It never grants Ready or native privilege.
    /// # Errors
    /// Rejects another client/generation, absent original record, missing genuine
    /// managed exit, changed source or independent cleanup failure. A never-Ready
    /// record returns None; metadata cannot be constructed from policy alone.
    pub(crate) fn historical_replacement_context(
        &self,
        binding: ClientBinding,
        current: LeaseKey,
        source: &FrontendInstallationInput,
    ) -> Result<Option<BackendReplacementContext<'_>>> {
        let record = self.historical_record(binding, current)?;
        if !record.source.matches_selection(source) {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        Ok(record
            .ready_context
            .as_ref()
            .map(|context| BackendReplacementContext { context, source: &record.source }))
    }

    /// Copy actual dead-B Ready matching data for bounded producer delivery.
    /// This performs no recipient source comparison and grants no Ready/native
    /// authority. Before accepting a handoff the recipient must separately call
    /// `historical_replacement_context` with its original source and authenticate
    /// its retained actual A-to-B event, B notice, old-A absence and native binding.
    /// # Errors
    /// Rejects the same foreign/current-B, absent-record, missing managed exit and
    /// independent cleanup failures as the source-checking historical query.
    pub(crate) fn historical_replacement_metadata(
        &self,
        binding: ClientBinding,
        current: LeaseKey,
    ) -> Result<Option<BackendReadyMetadata>> {
        Ok(self.historical_record(binding, current)?.ready_context)
    }

    fn historical_record(&self, binding: ClientBinding, current: LeaseKey) -> Result<&Backend> {
        // This is a separately successful current-B check, never a stale-A catch.
        self.policy.invalidated_lease(binding, current)?;
        let record = self.record(current.resource)?;
        if record.managed_exit.is_none()
            || !matches!(record.target, BackendTarget::Managed(_))
            || self.cleanup_failure(current.resource).is_some()
        {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        if record
            .ready_context
            .as_ref()
            .is_some_and(|context| context.resource() != current.resource)
        {
            return Err(Box::new(LifecycleRejection::StaleIdentity));
        }
        Ok(record)
    }
}

/// Fixed immutable matching data copied only at the genuine Ready producer.
/// It supplies no live readiness, native handle, source lease or presentation grant.
/// The private constructor accepts actual verified source metadata; copying it keeps
/// every generation and version association without retaining a native source owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BackendReadyMetadata {
    resource: ResourceKey,
    origin: LoopbackOrigin,
    frontend: [u8; 64],
    frontend_len: u8,
    cube: [u8; 64],
    cube_len: u8,
}
impl BackendReadyMetadata {
    pub(super) fn capture(
        resource: ResourceKey,
        origin: LoopbackOrigin,
        proof: &VerifiedReadiness,
    ) -> Result<Self> {
        let manifest = proof.source().manifest();
        let frontend = manifest.frontend_version().as_bytes();
        let cube = manifest.cube_version().as_bytes();
        if frontend.is_empty() || frontend.len() > 64 || cube.is_empty() || cube.len() > 64 {
            return Err(Box::new(LifecycleRejection::Overloaded));
        }
        let mut value = Self {
            resource,
            origin,
            frontend: [0; 64],
            frontend_len: frontend.len() as u8,
            cube: [0; 64],
            cube_len: cube.len() as u8,
        };
        value.frontend[..frontend.len()].copy_from_slice(frontend);
        value.cube[..cube.len()].copy_from_slice(cube);
        Ok(value)
    }
    /// Return the exact actual Ready generation, never the current policy generation by inference.
    pub(crate) const fn resource(&self) -> ResourceKey {
        self.resource
    }
    /// Return the exact loopback origin accepted at actual Ready publication.
    pub(crate) const fn origin(&self) -> LoopbackOrigin {
        self.origin
    }
    /// Borrow the actual installed version without allocating a presentation string.
    /// # Errors
    /// Preserves a malformed UTF-8 cause rather than silently replacing metadata.
    pub(crate) fn frontend_version(&self) -> std::result::Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.frontend[..usize::from(self.frontend_len)])
    }
    /// Borrow the actual installed SugarCubes version; failure retains UTF-8 cause.
    pub(crate) fn sugarcubes_version(&self) -> std::result::Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.cube[..usize::from(self.cube_len)])
    }
}

/// Borrow a genuine historical Ready association; this type grants no native action.
pub(crate) struct BackendReplacementContext<'a> {
    context: &'a BackendReadyMetadata,
    source: &'a InstalledFrontendOwner,
}
impl BackendReplacementContext<'_> {
    /// Copy actual immutable data into the recipient's already-funded pending handoff.
    pub(crate) const fn metadata(&self) -> BackendReadyMetadata {
        *self.context
    }
    /// Compare the complete original installed selection without filesystem authority.
    pub(crate) fn matches_selection(&self, source: &FrontendInstallationInput) -> bool {
        self.source.matches_selection(source)
    }
}
