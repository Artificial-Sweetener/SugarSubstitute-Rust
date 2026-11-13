// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode into the native preclaimed write loan; Busy never renews the original header.
use super::types::WritePurpose;
use super::{ParentError, ParentLifecycle, ParentResources, Reply, Result};
use sugarsubstitute_contracts::lifecycle::LeaseKey;
use sugarsubstitute_supervision::lifecycle_protocol::{
    BackendPeer, GrantedContext, LeaseReference, OutputDelivery, ReplyOutcome, VersionLabel,
    WireHeader, WirePayload,
};

/// Copy matching facts only from the parent's current verified readiness owner.
fn peer(
    readiness: &sugarsubstitute_comfy::readiness_transport::VerifiedReadiness,
) -> Result<BackendPeer> {
    Ok(BackendPeer {
        process_id: std::num::NonZeroU32::new(readiness.process_id())
            .ok_or(ParentError::WrongPhase)?,
        creation_time_100ns: std::num::NonZeroU64::new(readiness.creation_time_100ns())
            .ok_or(ParentError::WrongPhase)?,
    })
}

pub(super) fn reference(lease: LeaseKey) -> LeaseReference {
    LeaseReference {
        resource: lease.resource.id,
        generation: lease.resource.generation,
        lease: lease.id,
    }
}
impl ParentLifecycle {
    pub(super) fn send_next(&mut self, resources: &ParentResources<'_, '_>) -> Result<()> {
        if self.write.is_some() {
            return Ok(());
        }
        if self.peer.is_some() && !self.welcome {
            let header = WireHeader {
                run: self.run,
                connection: Some(self.connection),
                request: None,
                deadline_qpc: self.original.ticks(),
            };
            self.write_message(
                resources,
                header,
                WirePayload::Welcome { qpc_frequency: self.original.frequency() },
                resources.session.control.original_deadline(),
                WritePurpose::Welcome,
            )?;
            return Ok(());
        }
        self.expire_acquisition(resources.execution.client.now()?)?;
        if matches!(
            resources.target,
            sugarsubstitute_contracts::lifecycle::BackendTarget::Managed(_)
        ) && let Some(acquisition) = &self.acquisition
            && let Some(lease) = acquisition.progress()
        {
            let header = acquisition.header;
            let now = resources.execution.client.now()?;
            let original =
                self.acquisition.as_mut().ok_or(ParentError::WrongPhase)?.progress_deadline(now)?;
            let now = resources.execution.client.now()?;
            if now >= original {
                self.expire_acquisition(now)?;
                return Err(sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeError::DeadlineExpired.into());
            }
            self.begin_output_sequence(lease)?;
            if self.write_message(
                resources,
                header,
                WirePayload::Reply(ReplyOutcome::Pending(reference(lease))),
                original,
                WritePurpose::AcquisitionProgress(header),
            )? {
                self.acquisition
                    .as_mut()
                    .ok_or(ParentError::WrongPhase)?
                    .progress_queued(header)?;
            }
            return Ok(());
        }
        let Some(index) = self.next_live_reply(resources.execution.client.now()?)? else {
            self.send_challenge(resources)?;
            self.send_update_status(resources)?;
            return Ok(());
        };
        resources.session.control.validate_current_admission()?;
        if let Some(Reply::BackendReplaced { pair, .. }) = self.requests.get(index)?.reply
            && !self.replacement_reply_ready(pair, resources)?
        {
            return Ok(());
        }
        let record = self.requests.get(index)?;
        let header = record.header;
        let until = self.requests.reply_deadline(index, resources.execution.client.now()?)?;
        let record = self.requests.get(index)?;
        let now = resources.execution.client.now()?;
        if now >= until {
            self.expire_acquisition(now)?;
            if self.acquisition.as_ref().is_some_and(|acquisition| acquisition.retiring())
                && self.requests.belongs_to(
                    index,
                    self.acquisition
                        .as_ref()
                        .ok_or(ParentError::WrongPhase)?
                        .header
                        .request
                        .ok_or(ParentError::WrongPhase)?,
                )?
            {
                self.requests.abandon(index)?;
                return Ok(());
            }
            return Err(sugarsubstitute_execution::windows_lifecycle_pipe::
                LifecyclePipeError::DeadlineExpired.into());
        }
        let reply = record.reply.ok_or(ParentError::WrongPhase)?;
        let payload = match reply {
            Reply::Registered => ReplyOutcome::Registered {
                backend_data_disposition: resources.backend_data_disposition,
            },
            Reply::Acknowledged => ReplyOutcome::Acknowledged,
            Reply::Folder { identity, next } => {
                ReplyOutcome::FolderChunkAccepted { identity, next_offset_units: next }
            }
            Reply::Environment(identity) => {
                if let Some(refresh) = &self.frontend_refresh {
                    refresh.proof(identity, self.lease.ok_or(ParentError::WrongPhase)?)?;
                }
                if self
                    .environment
                    .as_ref()
                    .is_none_or(|proof| !proof.validates(resources.profile, identity))
                {
                    return Err(ParentError::ForeignPeer.into());
                }
                ReplyOutcome::EnvironmentAuthorized(identity)
            }
            Reply::Browser { identity, pid } => {
                if let Some(refresh) = &self.frontend_refresh {
                    refresh.proof(identity, self.lease.ok_or(ParentError::WrongPhase)?)?;
                }
                if self
                    .browser
                    .as_ref()
                    .is_none_or(|proof| !proof.validates(resources.profile, identity, pid.get()))
                {
                    return Err(ParentError::ForeignPeer.into());
                }
                ReplyOutcome::BrowserAuthorized { identity, browser_process_id: pid }
            }
            Reply::Granted { lease, identity } => {
                let readiness =
                    resources.backend.readiness(lease.resource).ok_or(ParentError::WrongPhase)?;
                readiness.validate_current()?;
                let source = readiness.source().manifest();
                ReplyOutcome::Granted(GrantedContext {
                    lease: reference(lease),
                    identity,
                    origin: resources
                        .backend
                        .ready_origin(lease.resource)?
                        .ok_or(ParentError::WrongPhase)?,
                    frontend: VersionLabel::new(source.frontend_version())?,
                    sugarcubes: VersionLabel::new(source.cube_version())?,
                    backend_peer: peer(readiness)?,
                    backend_binding: resources.backend.backend_binding(lease.resource)?,
                    installation: resources.installation.as_str(),
                    backend_data_disposition: resources.backend_data_disposition,
                })
            }
            Reply::Released => ReplyOutcome::Released,
            Reply::Reloaded { lease, identity } => {
                let refresh = self.frontend_refresh.as_ref().ok_or(ParentError::WrongPhase)?;
                let proof = refresh.proof(identity, lease)?;
                let source = proof.source().manifest();
                ReplyOutcome::Reloaded(GrantedContext {
                    lease: reference(lease),
                    identity,
                    origin: resources
                        .backend
                        .ready_origin(lease.resource)?
                        .ok_or(ParentError::WrongPhase)?,
                    frontend: VersionLabel::new(source.frontend_version())?,
                    sugarcubes: VersionLabel::new(source.cube_version())?,
                    backend_peer: peer(proof)?,
                    backend_binding: resources.backend.backend_binding(lease.resource)?,
                    installation: resources.installation.as_str(),
                    backend_data_disposition: resources.backend_data_disposition,
                })
            }
            Reply::Cancelled => ReplyOutcome::Cancelled,
            Reply::Failed(code) => ReplyOutcome::Failed(code),
            Reply::Rejected(code) => ReplyOutcome::Rejected(code),
            Reply::Output(facts) => ReplyOutcome::OutputDelivery(OutputDelivery {
                lease: reference(facts.lease),
                delivery_id: facts.id,
                stream: facts.stream,
                sequence: facts.sequence,
                gap: facts.gap,
                end: facts.end,
                bytes: &record.output_bytes[..facts.count],
            }),
            Reply::OutputIdle(lease) => ReplyOutcome::OutputIdle(reference(lease)),
            Reply::MainFailed(failure) => ReplyOutcome::MainStageFailed(failure),
            Reply::BackendUnavailable { failed, status, affected_clients, retry_ready } => {
                ReplyOutcome::BackendUnavailable {
                    failed: reference(failed),
                    status,
                    affected_clients,
                    retry_ready,
                }
            }
            Reply::BackendReplaced { identity, pair } => ReplyOutcome::BackendReplaced {
                identity,
                previous: reference(pair.previous),
                current: reference(pair.current),
            },
        };
        // The codec borrows record output; splitting native queue state avoids a copy.
        let control = &resources.session.control;
        let Some(mut buffer) = control.take_write_buffer()? else {
            return Ok(());
        };
        let bytes = sugarsubstitute_supervision::lifecycle_protocol::FrameCodec::encode(
            &sugarsubstitute_supervision::lifecycle_protocol::WireMessage {
                header,
                payload: WirePayload::Reply(payload),
            },
            buffer.as_mut_bytes(),
        )?;
        self.queue_encoded(resources, buffer, bytes, until, WritePurpose::Reply(index))?;
        Ok(())
    }
}
