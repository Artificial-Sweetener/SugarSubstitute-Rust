// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode into the native preclaimed write loan; Busy never renews the original header.
use super::types::WritePurpose;
use super::{ParentError, ParentLifecycle, ParentResources, Reply, Result};
use sugarsubstitute_contracts::lifecycle::LeaseKey;
use sugarsubstitute_supervision::lifecycle_protocol::{
    GrantedContext, LeaseReference, OutputDelivery, ReplyOutcome, VersionLabel, WireHeader,
    WirePayload,
};

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
            let (header, original) = (acquisition.header, acquisition.original);
            let now = resources.execution.client.now()?;
            if now >= original {
                self.expire_acquisition(now)?;
                return Ok(());
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
            return Ok(());
        };
        resources.session.control.validate_current_admission()?;
        let record = self.requests.get(index)?;
        let header = record.header;
        let until = record.until.ok_or(ParentError::WrongPhase)?;
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
            Reply::Acknowledged => ReplyOutcome::Acknowledged,
            Reply::Folder { identity, next } => {
                ReplyOutcome::FolderChunkAccepted { identity, next_offset_units: next }
            }
            Reply::Environment(identity) => {
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
                })
            }
            Reply::Released => ReplyOutcome::Released,
            Reply::Reloaded(lease) => ReplyOutcome::Reloaded(reference(lease)),
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
