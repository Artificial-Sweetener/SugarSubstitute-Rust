// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit complete authenticated frames before allocating or invoking actual owners.
use super::{ParentError, ParentLifecycle, ParentResources, Reply, Result};
use sugarsubstitute_application::supervision::MainAuthorizationObservation as Main;
use sugarsubstitute_contracts::lifecycle::{
    BackendTarget, ClientBinding, DesktopObservation, LeaseKey, ResourceCommand, ResourceRequest,
};
use sugarsubstitute_supervision::lifecycle_protocol::{
    DecodedPayload, FrameCodec, LeaseReference, WireHeader,
};

impl ParentLifecycle {
    /// Consume an authentic frame, or retain it before admission while shared output is loaned.
    /// False requires the caller to keep this exact original frame and its finite cutoff.
    pub(super) fn receive(
        &mut self,
        bytes: &[u8],
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<bool> {
        if self.peer.is_some() {
            resources.session.control.validate_current_admission()?;
        }
        let decoded = FrameCodec::decode(bytes)?;
        let header = decoded.header;
        let startup_operation = match &decoded.payload {
            DecodedPayload::Acquire { .. } => Some("Acquire"),
            DecodedPayload::SettleAttempt { .. } => Some("SettleAttempt"),
            DecodedPayload::RequestedProfileFolder(_) => Some("RequestedProfileFolder"),
            DecodedPayload::EnvironmentPrepared(_) => Some("EnvironmentPrepared"),
            DecodedPayload::ControllerPrepared { .. } => Some("ControllerPrepared"),
            DecodedPayload::MainRetired(_) => Some("MainRetired"),
            DecodedPayload::MainRevealed(_) => Some("MainRevealed"),
            DecodedPayload::MainFailed(_) => Some("MainFailed"),
            DecodedPayload::Release(_) => Some("Release"),
            DecodedPayload::ExitIntent { .. } => Some("ExitIntent"),
            DecodedPayload::ExitCompleted(_) => Some("ExitCompleted"),
            _ => None,
        };
        if let Some(startup_operation) = startup_operation {
            tracing::info!(operation="parent_startup_request_received",startup_operation,
                request=?header.request,run=?self.run,main_stage=?self.main.stage(),
                main_identity=?self.main.snapshot().identity,profile_pending=self.profile_pending.is_some(),
                native_profile_pending=resources.profile.snapshot().pending,
                "Actual authenticated channel startup request observed before semantic admission");
        }
        if header.run != self.run {
            return Err(ParentError::ForeignPeer.into());
        }
        if !self.hello {
            let DecodedPayload::Hello(capability) = decoded.payload else {
                return Err(ParentError::WrongPhase.into());
            };
            if header.connection.is_some()
                || header.request.is_some()
                || header.deadline_qpc != self.original.ticks()
                || !self.connected
            {
                return Err(ParentError::ForeignPeer.into());
            }
            resources.session.control.verify_capability(
                *capability.bytes(),
                resources.session.control.original_deadline(),
            )?;
            self.hello = true;
            return Ok(true);
        }
        if self.peer.is_none() || header.connection != Some(self.connection) {
            return Err(ParentError::ForeignPeer.into());
        }
        if self.requests.inspect(header, bytes)?.1
            && self.defer_output_receive(header, &decoded.payload, resources)?
        {
            return Ok(false);
        }
        let (index, fresh) = self.requests.admit(header, bytes)?;
        if !fresh {
            self.requests.get_mut(index)?.sent = false;
            return Ok(true);
        }
        if let DecodedPayload::OutputPull { lease, acknowledged_delivery } = decoded.payload
            && self.lease.is_none()
        {
            let supplied = resources.execution.client.decode_counter_deadline(
                sugarsubstitute_contracts::execution::CounterDeadline::new(
                    header.deadline_qpc,
                    self.original.frequency(),
                )?,
            )?;
            if self.retire_expired_output(
                index,
                self.request_binding(header)?,
                lease,
                acknowledged_delivery,
                supplied,
                resources.execution.client.now()?,
            )? {
                return Ok(true);
            }
        }
        let mut until = if !self.registered && matches!(decoded.payload, DecodedPayload::Register) {
            self.registration_deadline(header, resources)?
        } else if matches!(decoded.payload, DecodedPayload::OutputPull { .. }) {
            self.output_deadline(header, resources)?
        } else if matches!(decoded.payload, DecodedPayload::UiResponse(_)) {
            resources.execution.client.decode_counter_deadline(
                sugarsubstitute_contracts::execution::CounterDeadline::new(
                    header.deadline_qpc,
                    self.original.frequency(),
                )?,
            )?
        } else if matches!(
            decoded.payload,
            DecodedPayload::RequestedProfileFolder(_)
                | DecodedPayload::EnvironmentPrepared(_)
                | DecodedPayload::ControllerPrepared { .. }
        ) {
            self.frontend_deadline(header, resources)?
        } else {
            self.deadline(header, resources)?
        };
        until = self.cleanup_control_deadline(&decoded.payload, until);
        if matches!(decoded.payload, DecodedPayload::OutputPull { .. }) && self.lease.is_none() {
            until = self.acquisition.as_ref().ok_or(ParentError::WrongPhase)?.output_until(until);
            if resources.execution.client.now()? >= until {
                return Err(sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeError::DeadlineExpired.into());
            }
        }
        self.requests.get_mut(index)?.until = Some(until);
        if !self.registered {
            if !self.welcome
                || !matches!(decoded.payload, DecodedPayload::Register)
                || header.deadline_qpc != self.original.ticks()
            {
                return Err(ParentError::WrongPhase.into());
            }
            self.register(resources)?;
            self.requests.get_mut(index)?.reply = Some(Reply::Registered);
            return Ok(true);
        }
        let binding = self.binding()?;
        let request = header.request.ok_or(ParentError::WrongPhase)?;
        match decoded.payload {
            DecodedPayload::Acquire { identity, target } => {
                if self.cleanup.is_some()
                    || self.acquisition.is_some()
                    || self.attempt_cleanup.is_some()
                    || self.main.stage()
                        != sugarsubstitute_application::supervision::MainAuthorizationStage::Retired
                {
                    return Err(ParentError::WrongPhase.into());
                }
                if identity.run != self.run || target.to_domain()? != *resources.target {
                    return Err(ParentError::ForeignPeer.into());
                }
                self.requests.get_mut(index)?.identity = Some(identity);
                self.requests.acquisition(index, request)?;
                self.acquisition =
                    Some(super::acquisition::Acquisition::new(binding, header, identity, until)?);
                self.acquisition.as_mut().ok_or(ParentError::WrongPhase)?.wait_for_readiness();
                self.requests.defer_reply(index)?;
                self.hold_backend_command(
                    index,
                    super::history::HeldBackendCommand::Acquire(identity),
                )?;
                self.advance_backend_commands(resources)?;
            }
            DecodedPayload::RetryBackend { identity, failed } => {
                super::recovery::Recovery::main_identity(self, identity)?;
                if self
                    .recovery
                    .outage
                    .is_none_or(|outage| super::replies::reference(outage.failed) != failed)
                {
                    return Err(ParentError::ForeignPeer.into());
                }
                if !matches!(resources.target, BackendTarget::Managed(_)) {
                    return Err(ParentError::WrongPhase.into());
                }
                self.requests.defer_reply(index)?;
                self.hold_backend_command(
                    index,
                    super::history::HeldBackendCommand::Retry { identity, failed },
                )?;
                self.advance_backend_commands(resources)?;
            }
            DecodedPayload::SettleAttempt { identity, acquisition } => {
                self.begin_attempt_cleanup(index, identity, acquisition, until, resources)?;
            }
            DecodedPayload::Release(reference) => {
                if let Some(failed) = self.recovery.invalidated_release(reference)? {
                    resources.backend.invalidated_lease(binding, failed)?;
                    // Invalidation already removed the live lease. Keep its cause and
                    // retry lineage until the authenticated run disconnects.
                    self.requests.get_mut(index)?.reply = Some(Reply::Released);
                    return Ok(true);
                }
                let lease = self.lease(reference)?;
                self.hold_backend_command(
                    index,
                    super::history::HeldBackendCommand::Release(lease),
                )?;
                self.advance_backend_commands(resources)?;
            }
            DecodedPayload::Reload { lease: reference, identity } => {
                let policy = self.recovery;
                policy.reload(reference, |lease| {
                    self.hold_backend_command(
                        index,
                        super::history::HeldBackendCommand::Reload { identity, lease },
                    )?;
                    self.advance_backend_commands(resources)
                })?;
            }
            DecodedPayload::RequestedProfileFolder(fragment) => {
                let next = self.folders.requested(fragment, resources.requested_profile)?;
                self.main.accept(
                    binding,
                    request,
                    Main::RequestedFolderAccepted {
                        identity: fragment.identity(),
                        next_offset_units: next,
                    },
                    resources.execution.client.now()?,
                )?;
                self.requests.get_mut(index)?.reply =
                    Some(Reply::Folder { identity: fragment.identity(), next });
            }
            DecodedPayload::EnvironmentPrepared(fragment) => {
                if self.folders.effective(fragment, resources.requested_profile.len())? {
                    let effect = self.main.accept(
                        binding,
                        request,
                        Main::EnvironmentFolderComplete { identity: fragment.identity() },
                        resources.execution.client.now()?,
                    )?;
                    self.profile_effect(index, effect, until, resources)?;
                } else {
                    self.requests.get_mut(index)?.reply = Some(Reply::Folder {
                        identity: fragment.identity(),
                        next: fragment.offset_units() + u32::try_from(fragment.units().unit_len())?,
                    });
                }
            }
            DecodedPayload::ControllerPrepared { identity, browser_process_id } => {
                let effect = self.main.accept(
                    binding,
                    request,
                    Main::ControllerObserved { identity, browser_process_id },
                    resources.execution.client.now()?,
                )?;
                self.profile_effect(index, effect, until, resources)?;
            }
            DecodedPayload::MainRetired(identity) => {
                self.native_main_retired(index, identity, resources)?;
            }
            DecodedPayload::MainRevealed(identity) => {
                self.main_revealed(identity, binding, request, resources)?;
                self.requests.get_mut(index)?.reply = Some(Reply::Acknowledged);
            }
            DecodedPayload::NativeActivity { revision, activity } => {
                if revision.get() <= self.activity_revision {
                    return Err(ParentError::StaleRequest.into());
                }
                self.ui_activity(activity, resources.execution.client.now()?)?;
                self.activity_revision = revision.get();
                if let Some(write) = &self.write
                    && matches!(write.purpose, super::types::WritePurpose::Challenge(_))
                {
                    let _fact = write.admission.revoke();
                }
                self.requests.get_mut(index)?.reply = Some(Reply::Acknowledged);
            }
            DecodedPayload::UiResponse(id) => {
                self.ui_response(header, id, resources)?;
                self.requests.get_mut(index)?.reply = Some(Reply::Acknowledged);
            }
            DecodedPayload::ExitIntent { shutdown, intent, expected_code } => {
                let budget = self.exit.as_mut().ok_or(ParentError::WrongPhase)?.begin(
                    self.run,
                    shutdown,
                    intent,
                    i64::from(expected_code),
                    resources.execution.client.now()?,
                )?;
                self.cleanup = Some(budget);
                self.requests.get_mut(index)?.reply = Some(Reply::Acknowledged);
            }
            DecodedPayload::ExitCompleted(shutdown) => {
                self.exit
                    .as_mut()
                    .ok_or(ParentError::WrongPhase)?
                    .observe(DesktopObservation::Completed { run: self.run, shutdown })
                    .map_err(|rejected| rejected.reason)?;
                self.finish_disconnect(resources)?;
                self.requests.get_mut(index)?.reply = Some(Reply::Acknowledged);
                self.close_after_reply = Some(index);
            }
            DecodedPayload::OutputPull { lease, acknowledged_delivery } => {
                let original = self
                    .acquisition
                    .as_ref()
                    .ok_or(ParentError::WrongPhase)?
                    .header
                    .request
                    .ok_or(ParentError::WrongPhase)?;
                self.requests.acquisition(index, original)?;
                self.output_pull(index, lease, acknowledged_delivery, resources)?;
            }
            DecodedPayload::MainFailed(failure) => {
                self.main_failed(index, failure, resources)?;
            }
            _ => return Err(ParentError::WrongPhase.into()),
        }
        Ok(true)
    }

    pub(super) fn lease(&self, reference: LeaseReference) -> Result<LeaseKey> {
        self.recovery
            .live_lease()
            .or_else(|| self.recovery.running.is_none().then_some(self.lease).flatten())
            .filter(|lease| {
                reference.resource == lease.resource.id
                    && reference.generation == lease.resource.generation
                    && reference.lease == lease.id
            })
            .ok_or_else(|| ParentError::ForeignPeer.into())
    }

    pub(super) fn request_binding(&self, header: WireHeader) -> Result<ClientBinding> {
        if header.run != self.run || header.connection != Some(self.connection) {
            return Err(ParentError::ForeignPeer.into());
        }
        Ok(self.binding()?)
    }
}

impl ParentLifecycle {
    /// Reauthorize a new Main only after actual old retirement, retaining the healthy backend.
    pub(in crate::production::parent_lifecycle) fn native_reload(
        &mut self,
        index: usize,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        lease: LeaseKey,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        let binding = self.binding()?;
        if self.main.stage()
            != sugarsubstitute_application::supervision::MainAuthorizationStage::Retired
        {
            return Err(ParentError::WrongPhase.into());
        }
        self.acquisition.as_ref().ok_or(ParentError::WrongPhase)?.check_reload(
            identity,
            self.lease.ok_or(ParentError::WrongPhase)?,
            lease,
        )?;
        self.requests.get_mut(index)?.identity = Some(identity);
        self.backend_request(
            binding,
            ResourceRequest {
                run: self.run,
                connection: self.connection,
                id: self.requests.get(index)?.header.request.ok_or(ParentError::WrongPhase)?,
                command: ResourceCommand::Reload(lease),
            },
            resources,
        )?;
        self.acquisition.as_mut().ok_or(ParentError::WrongPhase)?.reload(
            identity,
            self.lease.ok_or(ParentError::WrongPhase)?,
            lease,
        )?;
        self.lease = Some(lease);
        Ok(())
    }
}
