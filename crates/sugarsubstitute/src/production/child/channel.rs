// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose the real inherited lifecycle transport without owning an executor.
use super::{handshake::WireHandshake, requests::ClientRequests};
use std::{error::Error, num::NonZeroUsize};
use sugarsubstitute_contracts::{
    execution::{
        CompletionBatch, CounterDeadline, JobClass, JobCompletion, JobId, JobSpec, Moment,
        Operation, Scope,
    },
    lifecycle::{RequestId, RunKey},
};
use sugarsubstitute_execution::{
    ExecutionClient,
    windows_lifecycle_pipe::{
        CapturedStartup, InheritedLifecycleBootstrap, LifecycleClientCapability,
        LifecyclePipeDeadline, LifecyclePipeError, LifecyclePipeFailure, LifecyclePipeObservation,
        LifecyclePipeOperationId, LifecyclePipeReport, LifecyclePipeSession,
        MIN_CAPTURED_LIFECYCLE_PIPE_BYTES, ReceivedLifecycleFrame, VerifiedLifecyclePeer,
    },
};
use sugarsubstitute_supervision::lifecycle_protocol::{
    CapabilityRef, FrameCodec, WireHeader, WireMessage, WirePayload,
};

#[path = "channel/admission.rs"]
mod admission;
#[path = "channel/application.rs"]
mod application;
#[path = "channel/challenge.rs"]
mod challenge;
#[path = "channel/commands.rs"]
mod commands;
#[path = "channel/expiry.rs"]
pub(super) mod expiry;
#[path = "channel/incoming.rs"]
mod incoming;
#[path = "channel/observations.rs"]
mod observations;
#[cfg(test)]
#[path = "channel/output_polling_fixture.rs"]
mod output_polling_fixture;
#[path = "channel/preparation.rs"]
pub(super) mod preparation;
#[path = "channel/retirement.rs"]
mod retirement;
#[path = "channel/ui.rs"]
mod ui;
#[path = "channel/wire.rs"]
mod wire;
use admission::ChannelAdmission;
use challenge::UiChallengeExchange;
use observations::{PendingRead, WriteReceipt};
#[path = "channel/write.rs"]
pub(super) mod write;
use write::PendingWrite;

type Result<T> = std::result::Result<T, Box<dyn Error>>;
type Terminal = JobCompletion<LifecyclePipeReport, LifecyclePipeFailure>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Exchange {
    Preparing,
    Welcome,
    Register,
    Application,
}

/// Own the inherited input, actual authenticated peer and original transport loan.
/// Native retirement and typed completion remain independent of wire registration.
pub(super) struct ChildChannel {
    input: Option<InheritedLifecycleBootstrap>,
    expected_run: RunKey,
    original: LifecyclePipeDeadline,
    session: Option<LifecyclePipeSession>,
    handshake: Option<WireHandshake>,
    requests: Option<ClientRequests>,
    ui: Option<UiChallengeExchange>,
    client: Option<ExecutionClient>,
    capability: Option<LifecycleClientCapability>,
    peer: Option<VerifiedLifecyclePeer>,
    exchange: Exchange,
    connected: bool,
    hello_pending: bool,
    hello_admitted: bool,
    register_admitted: bool,
    read: Option<PendingRead>,
    write: Option<PendingWrite>,
    latest: u64,
    received: Option<ReceivedLifecycleFrame>,
    admission: ChannelAdmission,
    terminal: Option<Terminal>,
    capture_input: preparation::CaptureInput,
}
impl ChildChannel {
    /// Retain the lexical inherited token without any native operation.
    pub(super) fn new(
        input: InheritedLifecycleBootstrap,
        original: LifecyclePipeDeadline,
        expected_run: RunKey,
    ) -> Self {
        Self {
            input: Some(input),
            expected_run,
            original,
            session: None,
            handshake: None,
            requests: None,
            ui: None,
            client: None,
            capability: None,
            peer: None,
            exchange: Exchange::Preparing,
            connected: false,
            hello_pending: false,
            hello_admitted: false,
            register_admitted: false,
            read: None,
            write: None,
            latest: 0,
            received: None,
            admission: ChannelAdmission::new(),
            terminal: None,
            capture_input: preparation::CaptureInput::Unchecked,
        }
    }

    /// Admit through the composing owner's scope before transferring the inherited input.
    /// Rejected reservation leaves the identical token here for parent-observed exit.
    pub(super) fn start(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        job: JobId,
    ) -> Result<()> {
        if self.session.is_some() || !self.admission.open() || self.input.is_none() {
            return Err("Production lifecycle channel cannot start twice or after shutdown".into());
        }
        let permit = client.reserve_captured_lifecycle_pipe_client(
            scope,
            JobSpec {
                id: job,
                operation: Operation::NativeLifecycle,
                class: JobClass::NativeObservation,
                deadline: None,
                reservation_bytes: MIN_CAPTURED_LIFECYCLE_PIPE_BYTES,
                replacement: None,
            },
            self.original,
        )?;
        let input =
            self.input.take().ok_or("Original lifecycle input disappeared after admission")?;
        self.session = permit.bind(input);
        if self.session.is_none() {
            return Err("Admitted lifecycle reservation lacks its original native session".into());
        }
        self.client = Some(client.clone());
        Ok(())
    }

    /// Capture matching terminal metadata before fallible observation handling.
    /// Retain the complete typed result and its original charge when acknowledged.
    pub(super) fn consume(&mut self, batch: &CompletionBatch) -> Result<()> {
        let Some(session) = &mut self.session else {
            return Ok(());
        };
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id)
                == (session.completion.scope_id(), session.completion.spec().id)
            {
                self.admission.observe_terminal(notice.kind)?;
                if let Some(requests) = &mut self.requests {
                    requests.close();
                }
                if let Some(ui) = &mut self.ui {
                    ui.close();
                }
            }
        }
        if self.terminal.is_none()
            && self.admission.terminal().is_some()
            && let Some(completion) = session.completion.try_take_completion()
        {
            let matches = completion.scope_id == session.completion.scope_id()
                && completion.spec == session.completion.spec()
                && Some(completion.outcome.kind()) == self.admission.terminal();
            self.terminal = Some(completion);
            if !matches {
                return Err("Production lifecycle typed terminal identity differs".into());
            }
        }
        Ok(())
    }

    /// Drain at most32 actual native observations, then admit eligible bounded work.
    /// No callback creates a second runtime, waits for a worker or allocates a frame.
    pub(super) fn activity(&mut self, max: NonZeroUsize) -> Result<()> {
        if max.get() > 32 {
            return Err("Production lifecycle observation bound exceeds32".into());
        }
        for _ in 0..max.get() {
            let next = self.session.as_ref().and_then(|session| session.control.take_observation());
            let Some(next) = next else { break };
            self.observe(next)?;
        }
        if self.admission.open() {
            self.advance()?;
        }
        Ok(())
    }

    /// Borrow genuine historical peer facts while this channel remains active.
    /// Actual native-effect owners revalidate current execution/source eligibility.
    pub(super) const fn peer(&self) -> Option<VerifiedLifecyclePeer> {
        if !self.admission.open() { None } else { self.peer }
    }

    /// Observe actual successful registration on the same authenticated channel.
    /// Kernel authentication or Welcome alone cannot establish this wire phase.
    pub(super) fn registered(&self) -> bool {
        self.admission.open() && self.exchange == Exchange::Application && self.peer.is_some()
    }

    /// Borrow completed wire binding; neither method grants browser/backend readiness.
    pub(super) fn binding(&self) -> Option<&WireHandshake> {
        if !self.admission.open() {
            return None;
        }
        self.handshake.as_ref().filter(|binding| binding.connection().is_some())
    }

    /// Transfer one original read guard without copying its charged frame.
    /// The next passive read remains blocked until this guard actually returns.
    pub(super) fn take_frame(&mut self) -> Option<ReceivedLifecycleFrame> {
        self.received.take()
    }

    /// Borrow typed terminal outcome with its unchanged original memory/native guards.
    /// A failure is retained evidence, not successful native or process settlement.
    pub(super) const fn terminal(&self) -> Option<&Terminal> {
        self.terminal.as_ref()
    }

    /// Observe actual admission independently of transport terminal or registration.
    pub(super) fn admitted(&self) -> bool {
        self.session.is_some()
    }

    /// Fence fresh commands and request the same transport's checked retirement.
    /// This neither renews a deadline nor declares that inherited/native custody stopped.
    pub(super) fn request_close(&mut self) {
        self.admission.close();
        if let Some(requests) = &mut self.requests {
            requests.close();
        }
        if let Some(ui) = &mut self.ui {
            ui.close();
        }
        if let Some(session) = &self.session {
            session.control.close();
        }
        self.received = None;
        self.capability = None;
    }

    /// Return the admitted local handshake cutoff, unchanged after authentication.
    pub(super) fn original_deadline(&self) -> Option<Moment> {
        self.session.as_ref().map(|session| session.control.original_deadline())
    }
}
