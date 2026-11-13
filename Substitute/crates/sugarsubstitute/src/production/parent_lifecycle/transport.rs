// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual issue/byte-count/terminal facts before releasing request records.
use super::retirement_trace::AckStage;
use super::types::{PendingWrite, WritePurpose};
use super::{ParentError, ParentLifecycle, ParentResources, Reply, Result};
use sha2::{Digest, Sha256};
use sugarsubstitute_application::supervision::UiWatchdog;
use sugarsubstitute_contracts::{
    execution::{CompletionBatch, JobOutcome, Moment},
    lifecycle::ClientRegistration,
};
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    LifecyclePipeError, LifecyclePipeObservation, LifecyclePipeOperationId,
};
use sugarsubstitute_supervision::lifecycle_protocol::{
    FrameCodec, WireHeader, WireMessage, WirePayload,
};

#[path = "transport/expiry.rs"]
mod expiry;

impl ParentLifecycle {
    pub(super) fn observe(
        &mut self,
        observation: LifecyclePipeObservation,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        if self.closed {
            match observation {
                LifecyclePipeObservation::Prepared
                | LifecyclePipeObservation::Connected
                | LifecyclePipeObservation::Authenticated(_)
                | LifecyclePipeObservation::ReadPending { .. } => return Ok(()),
                LifecyclePipeObservation::Received { operation, frame } => {
                    if self.read == Some(operation.get()) {
                        self.read = None;
                    }
                    drop(frame);
                    return Ok(());
                }
                _ => {}
            }
        }
        match observation {
            LifecyclePipeObservation::Prepared => {
                if self.prepared || resources.session.control.prepared_run() != Some(self.run) {
                    return Err(ParentError::ForeignPeer.into());
                }
                self.prepared = true;
            }
            LifecyclePipeObservation::Connected => {
                if !self.prepared || self.connected || resources.process.owner != self.run.owner {
                    return Err(ParentError::ForeignPeer.into());
                }
                self.connected = true;
            }
            LifecyclePipeObservation::Authenticated(peer) => {
                if !self.hello
                    || self.peer.is_some()
                    || peer.run() != self.run
                    || peer.process() != Some(resources.process)
                {
                    return Err(ParentError::ForeignPeer.into());
                }
                self.peer = Some(peer);
                self.exit =
                    Some(sugarsubstitute_application::supervision::DesktopExitTransaction::new(
                        self.run,
                        resources.process,
                    )?);
                self.watchdog = Some(UiWatchdog::new(self.run, resources.execution.client.now()?));
            }
            LifecyclePipeObservation::ReadPending { operation } => {
                if self.read != Some(operation.get()) {
                    return Err(ParentError::WrongPhase.into());
                }
            }
            LifecyclePipeObservation::Received { operation, frame } => {
                self.observe_received(operation, frame, resources)?
            }
            LifecyclePipeObservation::WriteIssued { operation, at } => {
                let write = self.write.as_mut().ok_or(ParentError::WrongPhase)?;
                write.receipt.issue(operation.get())?;
                if self.closed {
                    return Ok(());
                }
                match write.purpose {
                    WritePurpose::Reply(index) => {
                        if let Some(acquisition) = &mut self.acquisition {
                            acquisition.observe_write(
                                self.requests.get(index)?.header,
                                super::acquisition::WriteObservation::Issued,
                            )?;
                        }
                        if let Err(cause) = self.authorization_issued(index, resources) {
                            let record = self.requests.get(index).ok();
                            let reply_kind = match record.and_then(|record| record.reply) {
                                Some(Reply::Environment(_)) => "Environment",
                                Some(Reply::Browser { .. }) => "Browser",
                                Some(_) => "Ordinary",
                                None => "Unknown",
                            };
                            tracing::error!(operation="parent_issued_authorization_failed",
                                native_operation=operation.get(),at=?at,reply_kind,
                                request=?record.and_then(|record| record.header.request),
                                deadline_qpc=record.map(|record| record.header.deadline_qpc),
                                "Actual issued reply authorization rejected; original cause retained");
                            return Err(cause);
                        }
                        self.trace_retirement_ack(
                            self.requests.get(index)?.header,
                            AckStage::Issued,
                            Some(operation.get()),
                        );
                    }
                    WritePurpose::Challenge(challenge) => {
                        self.challenge_issued(
                            challenge,
                            operation.get(),
                            at,
                            resources.execution.client.now()?,
                        )?;
                    }
                    WritePurpose::Welcome
                    | WritePurpose::AcquisitionProgress(_)
                    | WritePurpose::UpdateStatus => {}
                }
            }
            LifecyclePipeObservation::Written { operation, bytes } => {
                let write = self.write.as_ref().ok_or(ParentError::WrongPhase)?;
                write.receipt.complete(operation.get(), bytes)?;
                match write.purpose {
                    WritePurpose::Welcome => self.welcome = true,
                    WritePurpose::Reply(index) => {
                        if let Some(acquisition) = &mut self.acquisition {
                            acquisition.observe_write(
                                self.requests.get(index)?.header,
                                super::acquisition::WriteObservation::Written,
                            )?;
                        }
                        if let Some(attempt) = &mut self.attempt_cleanup {
                            attempt.retirement_written(self.requests.get(index)?.header);
                        }
                        self.requests.get_mut(index)?.sent = true;
                        self.trace_retirement_ack(
                            self.requests.get(index)?.header,
                            AckStage::Written,
                            Some(operation.get()),
                        );
                        self.recovery_written(index)?;
                        self.finish_attempt_write(
                            self.requests.get(index)?.header,
                            resources.execution.client.now()?,
                        )?;
                        if self.close_after_reply == Some(index) {
                            self.closed = true;
                            resources.session.control.close();
                        }
                    }
                    WritePurpose::AcquisitionProgress(header) => {
                        self.acquisition
                            .as_mut()
                            .ok_or(ParentError::WrongPhase)?
                            .progress_finished(header, true)?;
                    }
                    WritePurpose::Challenge(_) => {}
                    WritePurpose::UpdateStatus => self.updates.written(),
                }
                self.write = None;
                self.resume_acquisition_receive(resources)?;
            }
            LifecyclePipeObservation::WriteRevoked { operation, buffer } => {
                self.write
                    .as_ref()
                    .ok_or(ParentError::WrongPhase)?
                    .receipt
                    .revoked(operation.get())?;
                if let WritePurpose::Challenge(challenge) =
                    self.write.as_ref().ok_or(ParentError::WrongPhase)?.purpose
                {
                    self.challenge_revoked(challenge, operation.get())?;
                }
                if matches!(
                    self.write.as_ref().ok_or(ParentError::WrongPhase)?.purpose,
                    WritePurpose::UpdateStatus
                ) {
                    self.updates.revoked();
                }
                if let WritePurpose::AcquisitionProgress(header) =
                    self.write.as_ref().ok_or(ParentError::WrongPhase)?.purpose
                {
                    self.acquisition
                        .as_mut()
                        .ok_or(ParentError::WrongPhase)?
                        .progress_finished(header, false)?;
                }
                if let WritePurpose::Reply(index) =
                    self.write.as_ref().ok_or(ParentError::WrongPhase)?.purpose
                {
                    if let Some(acquisition) = &mut self.acquisition {
                        acquisition.observe_write(
                            self.requests.get(index)?.header,
                            super::acquisition::WriteObservation::Revoked,
                        )?;
                    }
                    self.requests.abandon(index)?;
                }
                drop(buffer);
                self.write = None;
                self.resume_acquisition_receive(resources)?;
            }
        }
        Ok(())
    }

    pub(super) fn next_operation(&self) -> Result<u64> {
        self.latest_operation.checked_add(1).ok_or_else(|| ParentError::IdentityExhausted.into())
    }

    pub(super) fn write_message(
        &mut self,
        resources: &ParentResources<'_, '_>,
        header: WireHeader,
        payload: WirePayload<'_>,
        until: Moment,
        purpose: WritePurpose,
    ) -> Result<bool> {
        if self.write.is_some() {
            return Ok(false);
        }
        resources.session.control.validate_current_admission()?;
        let control = &resources.session.control;
        let Some(mut buffer) = control.take_write_buffer()? else {
            return Ok(false);
        };
        let bytes = FrameCodec::encode(&WireMessage { header, payload }, buffer.as_mut_bytes())?;
        self.queue_encoded(resources, buffer, bytes, until, purpose)
    }

    pub(super) fn queue_encoded(
        &mut self,
        resources: &ParentResources<'_, '_>,
        mut buffer: sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeWriteBuffer,
        bytes: usize,
        until: Moment,
        purpose: WritePurpose,
    ) -> Result<bool> {
        let next = self.next_operation()?;
        let encoded: [u8; 32] = Sha256::digest(
            buffer.as_mut_bytes().get(..bytes).ok_or(LifecyclePipeError::InvalidMessage)?,
        )
        .into();
        let control = &resources.session.control;
        match control.write(LifecyclePipeOperationId::new(next)?, until, buffer, bytes) {
            Ok(admission) => {
                self.latest_operation = next;
                self.write = Some(PendingWrite {
                    receipt: super::write_receipt::WriteReceipt::new(next, bytes),
                    admission,
                    purpose,
                });
                if let WritePurpose::Reply(index) =
                    self.write.as_ref().ok_or(ParentError::WrongPhase)?.purpose
                    && let Some(acquisition) = &mut self.acquisition
                {
                    acquisition.observe_write(
                        self.requests.get(index)?.header,
                        super::acquisition::WriteObservation::Queued,
                    )?;
                }
                if let WritePurpose::Reply(index) =
                    self.write.as_ref().ok_or(ParentError::WrongPhase)?.purpose
                {
                    self.trace_retirement_ack(
                        self.requests.get(index)?.header,
                        AckStage::Queued,
                        Some(next),
                    );
                }
                Ok(true)
            }
            Err(rejection) if rejection.error == LifecyclePipeError::Busy => {
                drop(rejection.buffer);
                Ok(false)
            }
            Err(mut rejection) => {
                if rejection.error != LifecyclePipeError::DeadlineExpired {
                    return Err(rejection.error.into());
                }
                let now = resources.execution.client.now().map_err(|observation| {
                    expiry::RejectionObservationFailure {
                        rejected: rejection.error,
                        observation: observation.into(),
                    }
                })?;
                let Some(returned) = rejection.buffer.as_mut_bytes().get(..bytes) else {
                    return Err(rejection.error.into());
                };
                if self.retire_unissued_expired_write(
                    purpose,
                    until,
                    rejection.error,
                    returned,
                    encoded,
                    now,
                )? {
                    tracing::info!(operation="parent_original_write_cutoff_retired",native_operation=next,issued=false,
                        request=?self.acquisition.as_ref().and_then(|acquisition| acquisition.header.request),
                        until=?until,cause=?rejection.error,
                        "Original never-issued buffer returned at its unchanged cutoff; acquisition retained for real retirement");
                    drop(rejection.buffer);
                    return Ok(false);
                }
                Err(rejection.error.into())
            }
        }
    }

    pub(super) fn register(&mut self, resources: &mut ParentResources<'_, '_>) -> Result<()> {
        resources.backend.register_run(ClientRegistration {
            run: self.run,
            launch: self.launch,
            profile: self.profile.clone(),
            connection: self.connection,
        })?;
        self.registered = true;
        Ok(())
    }

    pub(super) fn latch_terminal(
        &mut self,
        batch: &CompletionBatch,
        resources: &ParentResources<'_, '_>,
    ) -> Result<()> {
        let job = &resources.session.completion;
        let expected = (job.scope_id(), job.spec());
        for notice in &batch.notifications {
            if notice.job_id == expected.1.id {
                if notice.scope_id != expected.0
                    || notice.replacement.is_some()
                    || self.expected_terminal.is_some()
                {
                    return Err(ParentError::WrongPhase.into());
                }
                self.expected_terminal = Some((expected.0, expected.1, notice.kind));
                self.closed = true;
            }
        }
        Ok(())
    }

    pub(super) fn consume_terminal(
        &mut self,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        let Some((scope, spec, kind)) = self.expected_terminal else {
            return Ok(());
        };
        // Continue adoption for a retained original terminal after a prior semantic failure.
        if self.terminal.is_none() {
            self.terminal = resources.session.completion.try_take_completion();
        }
        let Some(terminal) = &self.terminal else {
            return Ok(());
        };
        if terminal.scope_id != scope || terminal.spec != spec || terminal.outcome.kind() != kind {
            return Err(ParentError::WrongPhase.into());
        }
        if self.checked_retirement.is_none() {
            self.checked_retirement = resources.session.control.take_checked_retirement();
        }
        let Some(proof) = &self.checked_retirement else {
            return Ok(());
        };
        if !proof.matches(&resources.session.control)
            || proof.scope_id() != scope
            || proof.spec() != spec
            || proof.run() != Some(self.run)
        {
            return Err(ParentError::WrongPhase.into());
        }
        self.terminal_accepted = true;
        self.read = None;
        self.write = None;
        if !self.terminal_failure_reported {
            // Report once after adopting the independent physical proof. The first
            // cause and complete typed terminal remain retained through later cleanup.
            self.terminal_failure_reported = true;
            match &terminal.outcome {
                JobOutcome::Success(report) if report.run == Some(self.run) => {}
                JobOutcome::Cancelled if self.cleanup.is_some() => {}
                JobOutcome::Failure(cause) => {
                    self.activity_failure_origin =
                        super::types::ParentActivityFailureOrigin::DesktopPipeTerminal;
                    return Err(cause.clone().into());
                }
                _ => return Err(ParentError::OwnerFailed.into()),
            }
        }
        Ok(())
    }
}
