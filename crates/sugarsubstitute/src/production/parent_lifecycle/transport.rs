// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual issue/byte-count/terminal facts before releasing request records.
use super::types::{PendingWrite, WritePurpose};
use super::{ParentError, ParentLifecycle, ParentResources, Result};
use sugarsubstitute_application::supervision::UiWatchdog;
use sugarsubstitute_contracts::{
    execution::{CompletionBatch, CounterDeadline, JobOutcome, Moment},
    lifecycle::{ClientRegistration, UiWatchdogObservation},
};
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    LifecyclePipeError, LifecyclePipeObservation, LifecyclePipeOperationId,
};
use sugarsubstitute_supervision::lifecycle_protocol::{
    FrameCodec, WireHeader, WireMessage, WirePayload,
};

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
                if self.read != Some(operation.get()) || self.rejected_frame.is_some() {
                    self.rejected_frame = Some(frame);
                    return Err(ParentError::WrongPhase.into());
                }
                self.read = None;
                if let Err(error) = self.receive(frame.as_bytes(), resources) {
                    self.rejected_frame = Some(frame);
                    return Err(error);
                }
            }
            LifecyclePipeObservation::WriteIssued { operation, at } => {
                let write = self.write.as_mut().ok_or(ParentError::WrongPhase)?;
                write.receipt.issue(operation.get())?;
                if self.closed {
                    return Ok(());
                }
                match write.purpose {
                    WritePurpose::Reply(index) => self.authorization_issued(index, resources)?,
                    WritePurpose::Challenge(challenge) => {
                        let watchdog = self.watchdog.as_mut().ok_or(ParentError::WrongPhase)?;
                        if watchdog.snapshot().pending == Some(challenge) {
                            watchdog.observe(
                                self.run,
                                UiWatchdogObservation::Dispatched { challenge, at },
                                resources.execution.client.now()?,
                            )?;
                        }
                    }
                    WritePurpose::Welcome => {}
                }
            }
            LifecyclePipeObservation::Written { operation, bytes } => {
                let write = self.write.as_ref().ok_or(ParentError::WrongPhase)?;
                write.receipt.complete(operation.get(), bytes)?;
                match write.purpose {
                    WritePurpose::Welcome => self.welcome = true,
                    WritePurpose::Reply(index) => {
                        self.requests.get_mut(index)?.sent = true;
                        if self.close_after_reply == Some(index) {
                            self.closed = true;
                            resources.session.control.close();
                        }
                    }
                    WritePurpose::Challenge(_) => {}
                }
                self.write = None;
            }
            LifecyclePipeObservation::WriteRevoked { operation, buffer } => {
                self.write
                    .as_ref()
                    .ok_or(ParentError::WrongPhase)?
                    .receipt
                    .revoked(operation.get())?;
                drop(buffer);
                self.write = None;
            }
        }
        Ok(())
    }

    pub(super) fn receive_next(&mut self, resources: &ParentResources<'_, '_>) -> Result<()> {
        if self.read.is_some() || !self.connected || self.closed {
            return Ok(());
        }
        if self.hello && (!self.welcome || self.peer.is_none()) {
            return Ok(());
        }
        let next = self.next_operation()?;
        let control = &resources.session.control;
        let admitted = if self.registered {
            control.read_passive(LifecyclePipeOperationId::new(next)?)
        } else {
            control.read(LifecyclePipeOperationId::new(next)?, control.original_deadline())
        };
        match admitted {
            Ok(()) => {
                self.latest_operation = next;
                self.read = Some(next);
            }
            Err(LifecyclePipeError::Busy) => {}
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }

    fn next_operation(&self) -> Result<u64> {
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
        buffer: sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeWriteBuffer,
        bytes: usize,
        until: Moment,
        purpose: WritePurpose,
    ) -> Result<bool> {
        let next = self.next_operation()?;
        let control = &resources.session.control;
        match control.write(LifecyclePipeOperationId::new(next)?, until, buffer, bytes) {
            Ok(admission) => {
                self.latest_operation = next;
                self.write = Some(PendingWrite {
                    receipt: super::write_receipt::WriteReceipt::new(next, bytes),
                    admission,
                    purpose,
                });
                Ok(true)
            }
            Err(rejection) if rejection.error == LifecyclePipeError::Busy => {
                drop(rejection.buffer);
                Ok(false)
            }
            Err(rejection) => Err(rejection.error.into()),
        }
    }

    pub(super) fn deadline(
        &self,
        header: WireHeader,
        resources: &ParentResources<'_, '_>,
    ) -> Result<Moment> {
        let until = resources.execution.client.decode_counter_deadline(CounterDeadline::new(
            header.deadline_qpc,
            self.original.frequency(),
        )?)?;
        let now = resources.execution.client.now()?;
        let kind = match resources.target {
            sugarsubstitute_contracts::lifecycle::BackendTarget::Managed(_) => {
                sugarsubstitute_application::startup::ConnectionBudgetKind::Managed
            }
            sugarsubstitute_contracts::lifecycle::BackendTarget::Attached(_) => {
                sugarsubstitute_application::startup::ConnectionBudgetKind::Attached
            }
        };
        // This checks a supplied interval's bound. The accepted header/Moment are
        // cached once; this upper-bound calculation never replaces that cutoff.
        let maximum = sugarsubstitute_application::startup::ConnectionBudget::new(kind, now)?;
        if until > maximum.until() || now >= until {
            return Err(LifecyclePipeError::DeadlineExpired.into());
        }
        Ok(until)
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
        let Some(completion) = resources.session.completion.try_take_completion() else {
            return Ok(());
        };
        let valid = completion.scope_id == scope
            && completion.spec == spec
            && completion.outcome.kind() == kind;
        self.terminal = Some(completion);
        if !valid {
            return Err(ParentError::WrongPhase.into());
        }
        match &self.terminal.as_ref().ok_or(ParentError::WrongPhase)?.outcome {
            JobOutcome::Success(report) if report.run == Some(self.run) => {}
            JobOutcome::Cancelled if self.cleanup.is_some() => {}
            JobOutcome::Failure(cause) => return Err(cause.clone().into()),
            _ => return Err(ParentError::OwnerFailed.into()),
        }
        self.terminal_accepted = true;
        self.read = None;
        self.write = None;
        Ok(())
    }
}
