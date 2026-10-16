// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind the recovery-only profile purpose to its real Root grant and predecessor custody.
use super::*;
use sugarsubstitute_execution::RegisteredRecoveryGrant;
impl WindowsProfileLease {
    /// Acquire the successor only after the actual old profile and family have retired.
    /// The original finite setup cutoff and all profile/Job/watch reservations precede ACK.
    /// # Errors
    /// Rejects an unretired predecessor, foreign source/registry, changed run or actual admission.
    #[allow(clippy::too_many_arguments)]
    pub fn submit_recovery_custodied_acquire(
        &mut self,
        execution: &ExecutionClient,
        registry: &mut WindowsProcessRegistry,
        scope: &Scope,
        spec: JobSpec,
        parent: &crate::BackendCustodyParent,
        watch: (sugarsubstitute_contracts::execution::ScopeSpec, JobSpec),
        grant: &RegisteredRecoveryGrant,
        previous: &WindowsProfileLease,
    ) -> Result<JobId, ClaimAdmissionError> {
        self.check_recovery_predecessor(registry, grant, previous)?;
        self.submit_profile_custodied_acquire(
            execution,
            registry,
            scope,
            spec,
            parent,
            watch,
            Some(grant.clone()),
        )
    }
    /// Borrow the original prepared Job's passive inherited-token value for carrier argv.
    /// The actual Job remains owned by the profile and explicitly allowlisted by its creator.
    /// # Errors
    /// Rejects an unpublished profile, different grant or already transferred native owner.
    pub fn recovery_job_token(
        &self,
        grant: &RegisteredRecoveryGrant,
    ) -> Result<usize, ClaimAdmissionError> {
        grant.check().map_err(ClaimAdmissionError::Execution)?;
        let native = lock(&self.shared.native);
        let prepared = native
            .as_ref()
            .and_then(|state| state.desktop_custody.as_ref())
            .ok_or(ClaimAdmissionError::Closed)?;
        if prepared.recovery_grant.as_ref().is_none_or(|actual| actual.key() != grant.key())
            || prepared
                .published_ack
                .as_ref()
                .is_none_or(|ack| ack.receipt() != Receipt::ProfilePublished(prepared.key))
        {
            return Err(ClaimAdmissionError::Identity);
        }
        prepared.family.as_ref().and_then(|family| family.job).ok_or(ClaimAdmissionError::Closed)
    }
    /// Pre-admit the original carrier creation after real profile publication, before ACK.
    /// Rejected admission preserves the complete input and every prepared native owner.
    /// # Errors
    /// Rejects a reused launch, different grant, unretired predecessor or real queue capacity.
    #[allow(clippy::too_many_arguments)]
    pub fn reserve_recovery_desktop_launch(
        &mut self,
        execution: &ExecutionClient,
        registry: &mut WindowsProcessRegistry,
        scope: &Scope,
        creation: JobSpec,
        grant: &RegisteredRecoveryGrant,
        previous: &WindowsProfileLease,
        input: &mut Option<crate::WindowsLaunchInput>,
    ) -> Result<LaunchTicket, ClaimAdmissionError> {
        self.check_recovery_predecessor(registry, grant, previous)?;
        validate_grant(self.run, scope, creation, grant)?;
        if self.pending.is_some()
            || self.shared.active.load(Ordering::Acquire) != HELD
            || self.shared.closing.load(Ordering::Acquire)
            || !matches!(self.launch, LaunchState::Unattempted)
            || self.prepared.load(Ordering::Acquire) != 0
        {
            return Err(ClaimAdmissionError::Busy);
        }
        let value = input.as_mut().ok_or(ClaimAdmissionError::Specification)?;
        if value.profile_custody.is_none() {
            let mut native = lock(&self.shared.native);
            let state = native.as_mut().ok_or(ClaimAdmissionError::Closed)?;
            let prepared = state.desktop_custody.as_ref().ok_or(ClaimAdmissionError::Identity)?;
            if prepared.recovery_grant.as_ref().is_none_or(|actual| actual.key() != grant.key())
                || prepared
                    .published_ack
                    .as_ref()
                    .is_none_or(|ack| ack.receipt() != Receipt::ProfilePublished(prepared.key))
            {
                return Err(ClaimAdmissionError::Identity);
            }
            let memory = state.memory.as_ref().ok_or(ClaimAdmissionError::Identity)?.clone();
            value.profile_custody = Some(
                crate::registry::profile::prepared::PreparedDesktopCarrier::new(
                    &mut state.desktop_custody,
                    memory,
                )
                .map_err(|cause| {
                    ClaimAdmissionError::Native(ClaimError::new("recovery_profile_carrier", cause))
                })?,
            );
        }
        let ticket = registry
            .reserve_recovery_profile_creation(execution, scope, creation, grant, input)
            .map_err(ClaimAdmissionError::Registry)?;
        self.prepared.store(3, Ordering::Release);
        self.launch = LaunchState::Accepted(ticket);
        Ok(ticket)
    }
    /// Require actual pre-admitted carrier/UI watches, held creation input and published profile before ACK.
    /// # Errors
    /// Rejects foreign/stopped grants, changed predecessor custody or registry identity.
    pub fn recovery_launch_acknowledgement_allowed(
        &self,
        registry: &WindowsProcessRegistry,
        grant: &RegisteredRecoveryGrant,
        previous: &WindowsProfileLease,
    ) -> Result<bool, ClaimAdmissionError> {
        self.check_recovery_predecessor(registry, grant, previous)?;
        let LaunchState::Accepted(ticket) = self.launch else { return Ok(false) };
        if self.shared.active.load(Ordering::Acquire) != HELD
            || self.shared.closing.load(Ordering::Acquire)
            || self.pending.is_some()
        {
            return Ok(false);
        }
        registry
            .recovery_profile_creation_ready(ticket, grant)
            .map_err(ClaimAdmissionError::Registry)
    }
    /// Activate the same held worker only after actual predecessor retirement and exact durable ACK.
    /// # Errors
    /// Rejection retains the original held job/inputs in the registry for checked cancellation.
    pub fn begin_recovery_desktop_launch(
        &self,
        registry: &mut WindowsProcessRegistry,
        grant: &RegisteredRecoveryGrant,
        previous: &WindowsProfileLease,
        acknowledgement: sugarsubstitute_contracts::incident::DurableAcknowledgement,
    ) -> Result<(), ClaimAdmissionError> {
        self.check_recovery_predecessor(registry, grant, previous)?;
        let LaunchState::Accepted(ticket) = self.launch else {
            return Err(ClaimAdmissionError::Identity);
        };
        registry
            .begin_recovery_profile_creation(ticket, grant, acknowledgement)
            .map_err(ClaimAdmissionError::Registry)
    }
    fn check_recovery_predecessor(
        &self,
        registry: &WindowsProcessRegistry,
        grant: &RegisteredRecoveryGrant,
        previous: &WindowsProfileLease,
    ) -> Result<(), ClaimAdmissionError> {
        grant.check().map_err(ClaimAdmissionError::Execution)?;
        self.registry_matches(registry)?;
        previous.registry_matches(registry)?;
        if self.run != grant.key().run()
            || previous.run != grant.key().previous()
            || self.path != previous.path
            || !previous.snapshot().retirement_ready()
            || registry
                .retained_desktop(previous.run)
                .map_err(ClaimAdmissionError::Registry)?
                .is_some()
        {
            return Err(ClaimAdmissionError::Busy);
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn submit_profile_custodied_acquire(
        &mut self,
        execution: &ExecutionClient,
        registry: &mut WindowsProcessRegistry,
        scope: &Scope,
        spec: JobSpec,
        parent: &crate::BackendCustodyParent,
        watch: (sugarsubstitute_contracts::execution::ScopeSpec, JobSpec),
        grant: Option<sugarsubstitute_execution::RegisteredRecoveryGrant>,
    ) -> Result<JobId, ClaimAdmissionError> {
        if let Some(grant) = &grant {
            validate_grant(self.run, scope, spec, grant)?;
        }
        if self.acquire_attempted || !parent.established() {
            return Err(ClaimAdmissionError::Closed);
        }
        self.registry_matches(registry)?;
        self.validate_submission(scope, spec)?;
        if !parent.matches_execution(execution) {
            return Err(ClaimAdmissionError::Identity);
        }
        let (identity, key, native) = registry
            .reserve_profile_family(self.run, parent.key(), execution, watch.0, watch.1)
            .map_err(ClaimAdmissionError::Registry)?;
        let parent = parent.clone();
        let client = execution.clone();
        let path = self.path.clone();
        let run = self.run;
        let port = self.port.clone();
        self.submit(
            execution,
            scope,
            spec,
            move |state, context| {
                state.desktop_custody = Some(
                    PreparedDesktop::new(identity, key, parent, native, &client, context, grant)
                        .map_err(|cause| ClaimError::new("profile_prepare_desktop_job", cause))?,
                );
                if let Some(prepared) = &state.desktop_custody {
                    state.custody_release = Some(Release {
                        parent: prepared.parent.clone(),
                        key: prepared.key,
                        until: None,
                        nonce: None,
                        counter: None,
                        ack: None,
                    });
                }
                port.acquire(state, &path, run, context)
            },
            false,
        )?;
        self.acquire_attempted = true;
        Ok(spec.id)
    }
}
/// Keep matching run and finite job cutoff within the genuine original setup interval.
fn validate_grant(
    run: RunKey,
    scope: &Scope,
    spec: JobSpec,
    grant: &RegisteredRecoveryGrant,
) -> Result<(), ClaimAdmissionError> {
    grant.check().map_err(ClaimAdmissionError::Execution)?;
    if grant.key().run() != run
        || scope.spec().deadline.is_some_and(|until| until > grant.original_budget().until())
        || spec.deadline != Some(grant.original_budget().until())
    {
        return Err(ClaimAdmissionError::Identity);
    }
    Ok(())
}
