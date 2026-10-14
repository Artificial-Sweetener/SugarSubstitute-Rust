// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit complete authenticated frames before allocating or invoking actual owners.
use super::{ParentError, ParentLifecycle, ParentResources, Reply, Result};
use sugarsubstitute_application::supervision::MainAuthorizationObservation as Main;
use sugarsubstitute_contracts::lifecycle::{
    ClientBinding, DesktopObservation, LeaseKey, ResourceCommand, ResourceRequest,
};
use sugarsubstitute_supervision::lifecycle_protocol::{
    DecodedPayload, FrameCodec, LeaseReference, WireHeader,
};

impl ParentLifecycle {
    pub(super) fn receive(
        &mut self,
        bytes: &[u8],
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        if self.forwarded.is_some() {
            return Err(ParentError::Overloaded.into());
        }
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
            DecodedPayload::MainFailed(_) => Some("MainFailed"),
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
            return Ok(());
        }
        if self.peer.is_none() || header.connection != Some(self.connection) {
            return Err(ParentError::ForeignPeer.into());
        }
        let (index, fresh) = self.requests.admit(header, bytes)?;
        if !fresh {
            self.requests.get_mut(index)?.sent = false;
            return Ok(());
        }
        let mut until = if matches!(decoded.payload, DecodedPayload::UiResponse(_)) {
            resources.execution.client.decode_counter_deadline(
                sugarsubstitute_contracts::execution::CounterDeadline::new(
                    header.deadline_qpc,
                    self.original.frequency(),
                )?,
            )?
        } else {
            self.deadline(header, resources)?
        };
        if matches!(
            decoded.payload,
            DecodedPayload::RequestedProfileFolder(_)
                | DecodedPayload::EnvironmentPrepared(_)
                | DecodedPayload::ControllerPrepared { .. }
        ) {
            until = until.min(self.main.snapshot().frontend_until.ok_or(ParentError::WrongPhase)?);
        }
        if matches!(
            decoded.payload,
            DecodedPayload::MainRetired(_)
                | DecodedPayload::ExitIntent { .. }
                | DecodedPayload::ExitCompleted(_)
        ) && let Some(budget) = self.cleanup
        {
            until = until.min(budget.settle_until);
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
            self.requests.get_mut(index)?.reply = Some(Reply::Acknowledged);
            return Ok(());
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
                self.acquisition =
                    Some(super::acquisition::Acquisition::new(binding, header, identity, until)?);
                self.backend_request(
                    binding,
                    ResourceRequest {
                        run: self.run,
                        connection: self.connection,
                        id: request,
                        command: ResourceCommand::Acquire(resources.target.clone()),
                    },
                    resources,
                )?;
            }
            DecodedPayload::SettleAttempt { identity, acquisition } => {
                self.begin_attempt_cleanup(index, identity, acquisition, until, resources)?;
            }
            DecodedPayload::Release(reference) => {
                let lease = self.lease(reference)?;
                self.backend_request(
                    binding,
                    ResourceRequest {
                        run: self.run,
                        connection: self.connection,
                        id: request,
                        command: ResourceCommand::Release(lease),
                    },
                    resources,
                )?;
            }
            DecodedPayload::Reload { lease: reference, identity } => {
                let lease = self.lease(reference)?;
                if self.main.stage()
                    != sugarsubstitute_application::supervision::MainAuthorizationStage::Retired
                {
                    return Err(ParentError::WrongPhase.into());
                }
                self.requests.get_mut(index)?.identity = Some(identity);
                self.backend_request(
                    binding,
                    ResourceRequest {
                        run: self.run,
                        connection: self.connection,
                        id: request,
                        command: ResourceCommand::Reload(lease),
                    },
                    resources,
                )?;
                let frontend = sugarsubstitute_application::startup::FrontendBudget::new(
                    resources.execution.client.now()?,
                )?;
                self.main.begin(identity, lease, until.min(frontend.until()))?;
                self.acquisition
                    .as_mut()
                    .ok_or(ParentError::WrongPhase)?
                    .reload(identity, lease)?;
                resources.profile.begin_main_attempt(identity)?;
                self.folders.begin(identity);
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
                self.main.retire(identity)?;
                resources.profile.retire_main_attempt(identity)?;
                self.main.accept(
                    binding,
                    request,
                    Main::MainRetired { identity },
                    resources.execution.client.now()?,
                )?;
                self.environment = None;
                self.browser = None;
                self.main_retirement = Some((index, identity));
                self.finish_main_retirement(resources)?;
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
                self.output_pull(index, lease, acknowledged_delivery, resources)?;
            }
            DecodedPayload::MainFailed(failure) => {
                self.main_failed(index, failure, resources)?;
            }
            _ => return Err(ParentError::WrongPhase.into()),
        }
        Ok(())
    }

    pub(super) fn lease(&self, reference: LeaseReference) -> Result<LeaseKey> {
        self.lease
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
