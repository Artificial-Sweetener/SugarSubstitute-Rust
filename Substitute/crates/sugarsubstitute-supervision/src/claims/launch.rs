// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose immutable canonical input with the retained registry ownership boundary.
use super::*;
use crate::{
    ClaimLaunchJobs, FamilyBinding, LaunchEnvironment, LaunchTicket, ManagedLaunchInput,
    WindowsLaunchCommand, WindowsLaunchInput, WindowsProcessRegistry,
};
use std::os::windows::ffi::OsStrExt;
impl WindowsResourceClaims {
    /// Prepare one immutable input for an acquired resource, without native effects.
    /// The private capsule retains canonical proof until it is consumed or dropped.
    pub fn prepare_managed_launch(
        &mut self,
        resource: ResourceKey,
        environment: LaunchEnvironment,
    ) -> Result<ManagedLaunchInput, ClaimAdmissionError> {
        self.usable()?;
        let index = self.backend_index(resource)?;
        let entry = self.backends[index].as_mut().ok_or(ClaimAdmissionError::Identity)?;
        if !entry.claimed
            || entry.pending
            || entry.failed
            || !matches!(entry.launch, LaunchState::Unattempted)
            || entry.prepared.load(Ordering::Acquire) == 1
        {
            return Err(ClaimAdmissionError::Busy);
        }
        let config = &entry.proof.configuration;
        let units = config.root().as_os_str().encode_wide().count()
            + config.interpreter().as_os_str().encode_wide().count();
        let argument_units =
            config.arguments().iter().map(|a| a.encode_utf16().count()).sum::<usize>();
        let peak = submission::configuration_bytes(config)
            + (units * 2 + argument_units * 2 + 32767 * 2) as u64
            + environment.allocated_bytes().map_err(|_| ClaimAdmissionError::Specification)?
            + 65536;
        if peak > pool::SLOT_BYTES {
            return Err(ClaimAdmissionError::Specification);
        }
        let arguments: Vec<Vec<u16>> =
            config.arguments().iter().map(|a| a.encode_utf16().collect()).collect();
        let command = WindowsLaunchCommand::from_utf16(
            &config.interpreter().as_os_str().encode_wide().collect::<Vec<_>>(),
            &config.root().as_os_str().encode_wide().collect::<Vec<_>>(),
            &arguments,
        )
        .map_err(|e| ClaimAdmissionError::Native(ClaimError::new("managed_command", e)))?;
        let mut input = WindowsLaunchInput::new(command, environment)
            .map_err(|e| ClaimAdmissionError::Native(ClaimError::new("managed_input", e)))?;
        if let Some(parent) = &self.custodian {
            input.managed_custody = Some(managed_custody::ManagedCustodySource::new(entry, parent));
        }
        entry.prepared.store(1, Ordering::Release);
        Ok(ManagedLaunchInput {
            resource,
            origin: self.origin.clone(),
            input: Some(input),
            prepared: entry.prepared.clone(),
            proof: entry.proof.clone(),
        })
    }
    /// Submit an immutable claim-bound launch through the actual process registry.
    /// Registry rejection is latched even if it retains an unbound watch awaiting retirement.
    pub fn submit_managed_launch(
        &mut self,
        execution: &ExecutionClient,
        registry: &mut WindowsProcessRegistry,
        scope: &Scope,
        jobs: ClaimLaunchJobs,
        mut input: ManagedLaunchInput,
    ) -> Result<LaunchTicket, ClaimAdmissionError> {
        self.usable()?;
        self.validate_scope(scope)?;
        if !Arc::ptr_eq(&input.origin, &self.origin) {
            return Err(ClaimAdmissionError::Identity);
        }
        let index = self.backend_index(input.resource)?;
        let entry = self.backends[index].as_mut().ok_or(ClaimAdmissionError::Identity)?;
        let identity =
            registry.claim_identity(input.resource.owner).map_err(ClaimAdmissionError::Registry)?;
        if !self.registry.as_ref().is_some_and(|old| old.same_as(&identity)) {
            return Err(ClaimAdmissionError::Identity);
        }
        registry.retained_managed(input.resource).map_err(ClaimAdmissionError::Registry)?;
        if !entry.claimed
            || entry.pending
            || entry.failed
            || !matches!(entry.launch, LaunchState::Unattempted)
            || !Arc::ptr_eq(&entry.prepared, &input.prepared)
            || !Arc::ptr_eq(&entry.proof, &input.proof)
            || entry.prepared.load(Ordering::Acquire) != 1
        {
            return Err(ClaimAdmissionError::Busy);
        }
        let native_input = input.input.take().ok_or(ClaimAdmissionError::Identity)?;
        entry.prepared.store(3, Ordering::Release);
        entry.registry = Some(identity.clone());
        match registry.submit_launch(
            execution,
            scope,
            jobs.native_scope,
            jobs.creation,
            jobs.root_watch,
            FamilyBinding::Managed(input.resource),
            native_input,
        ) {
            Ok(ticket) => {
                entry.launch = LaunchState::Accepted(ticket);
                Ok(ticket)
            }
            Err(cause) => {
                entry.launch = LaunchState::Rejected;
                Err(ClaimAdmissionError::Registry(cause))
            }
        }
    }
}
