// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the isolated child's authenticated codec exchange and actual read retirement.
use super::{
    Result, failure,
    options::{CaseKind, ChildOptions},
    protocol,
};
use std::{num::NonZeroUsize, sync::Arc};
use sugarsubstitute_contracts::{
    execution::{ExecutionPort, JobOutcome, Moment, Scope},
    lifecycle::{ConnectionId, RequestId, RunKey},
};
use sugarsubstitute_desktop::run_headless;
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    LifecycleClientCapability, LifecyclePipeFailure, LifecyclePipeObservation, LifecyclePipeSession,
};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock};
use sugarsubstitute_supervision::lifecycle_protocol::{
    CapabilityRef, DecodedPayload, FrameCodec, ReplyOutcome, WirePayload,
};

pub(super) fn run(options: ChildOptions) -> Result<i32> {
    let mut child = Child::new(options);
    run_headless(&mut child)?;
    if !child.finished
        || !child.successful_terminal
        || (child.options.kind != CaseKind::Valid && child.expected_failure.is_none())
    {
        return Err("Lifecycle child lacks actual successful qualification retirement".into());
    }
    Ok(i32::try_from(child.options.kind.exit_code())?)
}
pub(super) struct Child {
    pub(super) options: ChildOptions,
    pub(super) clock: Arc<MonotonicClock>,
    pub(super) owner: Option<ExecutionOwner>,
    pub(super) client: Option<ExecutionClient>,
    pub(super) scope: Option<Scope>,
    pub(super) session: Option<LifecyclePipeSession>,
    pub(super) deadline: Option<Moment>,
    pub(super) shutdown_requested: bool,
    pub(super) deadline_error: bool,
    pub(super) finished: bool,
    pub(super) consumed: bool,
    pub(super) successful_terminal: bool,
    run: Option<RunKey>,
    capability: Option<LifecycleClientCapability>,
    server_verified: bool,
    hello_pending: bool,
    hello_sent: bool,
    hello_written: bool,
    welcome: bool,
    register_sent: bool,
    register_written: bool,
    reply: bool,
    final_read: bool,
    final_pending: bool,
    intentional_close: bool,
    expected_failure: Option<LifecyclePipeFailure>,
    terminal_notice: super::terminal::PendingTerminal,
}
impl Child {
    fn new(options: ChildOptions) -> Self {
        Self {
            options,
            clock: MonotonicClock::new(),
            owner: None,
            client: None,
            scope: None,
            session: None,
            deadline: None,
            shutdown_requested: false,
            deadline_error: false,
            finished: false,
            consumed: false,
            successful_terminal: false,
            run: None,
            capability: None,
            server_verified: false,
            hello_pending: false,
            hello_sent: false,
            hello_written: false,
            welcome: false,
            register_sent: false,
            register_written: false,
            reply: false,
            final_read: false,
            final_pending: false,
            intentional_close: false,
            expected_failure: None,
            terminal_notice: Default::default(),
        }
    }
    /// Drain bounded metadata and observations; typed terminal owns actual native retirement.
    pub(super) fn activity(&mut self, max: NonZeroUsize) -> Result<()> {
        let Some(client) = &self.client else { return Ok(()) };
        let batch = client.drain_completions(max)?;
        if let Some(session) = &self.session {
            self.terminal_notice.capture(
                session.completion.scope_id(),
                session.completion.spec().id,
                &batch,
            )?;
        }
        for _ in 0..8 {
            let next = self.session.as_ref().and_then(|session| session.control.take_observation());
            let Some(next) = next else { break };
            self.observe(next)?;
        }
        self.advance()?;
        self.consume()
    }
    fn observe(&mut self, observation: LifecyclePipeObservation) -> Result<()> {
        let control = &self.session.as_ref().ok_or("Lifecycle child session missing")?.control;
        match observation {
            LifecyclePipeObservation::Prepared => {
                if self.run.is_some() {
                    return Err("Lifecycle child repeated native preparation".into());
                }
                self.run = control.prepared_run();
                self.capability = control.take_client_capability()?;
                if self.run.is_none() || self.capability.is_none() {
                    return Err("Checked child bootstrap lacks original run/capability".into());
                }
            }
            LifecyclePipeObservation::Connected => {
                let capability =
                    self.capability.as_ref().ok_or("Lifecycle child capability missing")?;
                control.verify_capability(*capability.bytes(), control.original_deadline())?;
            }
            LifecyclePipeObservation::Authenticated(peer) => {
                if self.server_verified || Some(peer.run()) != self.run || peer.process().is_some()
                {
                    return Err(
                        "Lifecycle coordinator identity differs from checked bootstrap".into()
                    );
                }
                self.server_verified = true;
                protocol::read(control, 1)?;
            }
            LifecyclePipeObservation::ReadPending { operation } => match operation.get() {
                1 if self.server_verified && !self.hello_pending => self.hello_pending = true,
                4 if self.register_sent => {}
                5 if self.final_read && !self.final_pending => {
                    self.final_pending = true;
                    self.intentional_close = true;
                    control.close();
                }
                _ => return Err("Lifecycle child pending-read identity/order differs".into()),
            },
            LifecyclePipeObservation::Written { operation, bytes } => {
                if bytes == 0 {
                    return Err("Lifecycle child observed empty write".into());
                }
                match operation.get() {
                    2 if self.hello_sent && !self.hello_written => self.hello_written = true,
                    3 if self.register_sent && !self.register_written => {
                        self.register_written = true
                    }
                    _ => return Err("Lifecycle child write identity/order differs".into()),
                }
            }
            LifecyclePipeObservation::Received { operation, frame } => {
                let decoded = FrameCodec::decode(frame.as_bytes())?;
                let run = self.run.ok_or("Lifecycle child checked run missing")?;
                let connection = Some(ConnectionId::new(1)?);
                match (operation.get(), decoded.payload) {
                    (1, DecodedPayload::Welcome { qpc_frequency })
                        if self.hello_sent
                            && !self.welcome
                            && self.options.kind == CaseKind::Valid =>
                    {
                        protocol::validate(
                            decoded.header,
                            protocol::header(run, connection, None, self.options.deadline.ticks()),
                        )?;
                        if qpc_frequency != self.options.deadline.frequency() {
                            return Err("Lifecycle Welcome changed original frequency".into());
                        }
                        self.welcome = true;
                    }
                    (4, DecodedPayload::Reply(ReplyOutcome::Acknowledged))
                        if self.register_sent && !self.reply =>
                    {
                        protocol::validate(
                            decoded.header,
                            protocol::header(
                                run,
                                connection,
                                Some(RequestId::new(1)?),
                                self.options.deadline.ticks(),
                            ),
                        )?;
                        self.reply = true;
                    }
                    _ => {
                        return Err("Lifecycle child received an unexpected codec operation".into());
                    }
                }
                // Dropping this guard returns the fixed read buffer before the next read.
                drop(frame);
            }
        }
        Ok(())
    }
    fn advance(&mut self) -> Result<()> {
        if self.shutdown_requested || self.consumed {
            return Ok(());
        }
        let control = &self.session.as_ref().ok_or("Lifecycle child session missing")?.control;
        let Some(run) = self.run else { return Ok(()) };
        if self.hello_pending && !self.hello_sent {
            let capability =
                self.capability.take().ok_or("Lifecycle child capability already consumed")?;
            let mut candidate = *capability.bytes();
            if self.options.kind == CaseKind::WrongCapability {
                candidate[0] ^= 1;
            }
            let mut claimed = run;
            if self.options.kind == CaseKind::ForeignGeneration {
                claimed.generation = run.generation.next()?;
            }
            let result = protocol::write(
                control,
                2,
                protocol::message(
                    protocol::header(claimed, None, None, self.options.deadline.ticks()),
                    WirePayload::Hello(CapabilityRef::new(&candidate)),
                ),
            );
            candidate.fill(0);
            result?;
            self.hello_sent = true;
        }
        if self.welcome && self.hello_written && !self.register_sent {
            protocol::write(
                control,
                3,
                protocol::message(
                    protocol::header(
                        run,
                        Some(ConnectionId::new(1)?),
                        Some(RequestId::new(1)?),
                        self.options.deadline.ticks(),
                    ),
                    WirePayload::Register,
                ),
            )?;
            protocol::read(control, 4)?;
            self.register_sent = true;
        }
        if self.reply && self.register_written && !self.final_read {
            protocol::read(control, 5)?;
            self.final_read = true;
        }
        Ok(())
    }
    fn consume(&mut self) -> Result<()> {
        let Some(session) = &mut self.session else { return Ok(()) };
        let job = &mut session.completion;
        let expected = (job.scope_id(), job.spec());
        let Some(kind) = self.terminal_notice.kind() else { return Ok(()) };
        let Some(completion) = job.try_take_completion() else { return Ok(()) };
        self.consumed = true;
        if completion.scope_id != expected.0
            || completion.spec != expected.1
            || completion.outcome.kind() != kind
        {
            return Err("Lifecycle child terminal identity differs".into());
        }
        match completion.outcome {
            JobOutcome::Cancelled
                if self.options.kind == CaseKind::Valid
                    && self.intentional_close
                    && self.final_pending
                    && self.reply
                    && self.register_written =>
            {
                self.successful_terminal = true
            }
            JobOutcome::Failure(error)
                if self.options.kind != CaseKind::Valid
                    && self.server_verified
                    && self.hello_pending
                    && self.hello_written
                    && failure::expected_disconnect(&error) =>
            {
                tracing::info!(
                    operation = "lifecycle_child_expected_denial",
                    case = self.options.kind.name(),
                    stage = error.native_cause().stage(),
                    win32_code = error.win32_code(),
                    "Contained child observed the expected denied peer closure"
                );
                self.expected_failure = Some(error);
                self.successful_terminal = true;
            }
            other => return Err(Box::new(failure::unexpected(other))),
        }
        Ok(())
    }
}
