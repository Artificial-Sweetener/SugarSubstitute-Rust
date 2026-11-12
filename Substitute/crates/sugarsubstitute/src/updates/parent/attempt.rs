// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The launching process's end of one update-attempt channel: authenticate the helper, then
//! validate and keep its progress and its one result. The parent sends only a cancel request.
use std::{fmt, num::NonZeroUsize, time::Duration};
use sugarsubstitute_contracts::{
    execution::{JobCompletion, JobSpec, Moment, Scope},
    lifecycle::ProcessKey,
    update::attempt::{
        AttemptOutcome, AttemptProgress,
        codec::{self, AttemptCodecError, AttemptMessage, UpdateAttemptChannel},
    },
};
use sugarsubstitute_execution::{
    ExecutionClient,
    windows_purpose_pipe::{
        LifecyclePipeAdmissionError, LifecyclePipeError, LifecyclePipeFailure,
        LifecyclePipeOperationId, LifecyclePipeWriteBuffer, PurposeEnrollment, PurposeKey,
        PurposePipeBootstrap, PurposePipeObservation, PurposePipeReport, PurposePipeSession,
    },
};

/// Time the helper has to start, connect and authenticate.
pub(in crate::updates) const HANDSHAKE: Duration = Duration::from_secs(60);
/// Bound of the parent's one cancel write.
const CANCEL_WRITE: Duration = Duration::from_secs(10);

/// Why the channel stopped being a trustworthy account of the attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::updates) enum AttemptChannelError {
    /// The transport rejected an operation or reported a protocol violation.
    Pipe(LifecyclePipeError),
    /// A frame failed validation.
    Frame(AttemptCodecError),
    /// A frame arrived after the result.
    AfterResult,
}
impl fmt::Display for AttemptChannelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pipe(cause) => write!(f, "Update attempt channel failed: {cause}"),
            Self::Frame(cause) => write!(f, "Update attempt channel rejected a frame: {cause}"),
            Self::AfterResult => f.write_str("Update helper wrote after its result"),
        }
    }
}
impl std::error::Error for AttemptChannelError {}
impl From<LifecyclePipeError> for AttemptChannelError {
    fn from(cause: LifecyclePipeError) -> Self {
        Self::Pipe(cause)
    }
}

/// Where the parent's single cancel request stands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Cancel {
    None,
    Wanted,
    Sent,
}

/// One attempt's channel; it owns its pipe session until observed retirement.
pub(in crate::updates) struct AttemptChannel {
    session: PurposePipeSession,
    channel: UpdateAttemptChannel,
    prepared: bool,
    pins: Option<(PurposePipeBootstrap, PurposeEnrollment)>,
    process: Option<ProcessKey>,
    authenticated: bool,
    verification_queued: bool,
    next_operation: u64,
    progress: Option<AttemptProgress>,
    outcome: Option<AttemptOutcome>,
    cancel: Cancel,
    buffer: Option<LifecyclePipeWriteBuffer>,
    failure: Option<AttemptChannelError>,
    terminal: Option<JobCompletion<PurposePipeReport, LifecyclePipeFailure>>,
}
impl AttemptChannel {
    /// Admit the parent side of `channel` through the execution owner. The helper must
    /// authenticate before `handshake`; the session itself has no deadline.
    /// # Errors
    /// Returns the execution owner's admission refusal.
    pub(in crate::updates) fn reserve(
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        channel: UpdateAttemptChannel,
        handshake: Moment,
    ) -> Result<Self, LifecyclePipeAdmissionError> {
        let session = client.reserve_purpose_pipe_server(
            scope,
            spec,
            PurposeKey::UpdateAttempt(channel),
            handshake,
        )?;
        Ok(Self {
            session,
            channel,
            prepared: false,
            pins: None,
            process: None,
            authenticated: false,
            verification_queued: false,
            next_operation: 0,
            progress: None,
            outcome: None,
            cancel: Cancel::None,
            buffer: None,
            failure: None,
            terminal: None,
        })
    }

    /// The bootstrap and enrollment pins, once the server endpoint exists. They go into the
    /// helper's launch input, which inherits the section and enrolls the suspended helper.
    pub(in crate::updates) fn take_pins(
        &mut self,
    ) -> Option<(PurposePipeBootstrap, PurposeEnrollment)> {
        self.pins.take()
    }

    /// The helper's registry identity; only that process may authenticate.
    /// # Errors
    /// Rejects a different process for the same channel.
    pub(in crate::updates) fn bind(
        &mut self,
        process: ProcessKey,
    ) -> Result<(), AttemptChannelError> {
        if self.process.is_some_and(|old| old != process) {
            return Err(LifecyclePipeError::InvalidOperation.into());
        }
        self.process = Some(process);
        Ok(())
    }

    /// Latest validated progress.
    pub(in crate::updates) const fn progress(&self) -> Option<AttemptProgress> {
        self.progress
    }

    /// The helper's validated result, once.
    pub(in crate::updates) fn take_outcome(&mut self) -> Option<AttemptOutcome> {
        self.outcome.take()
    }

    /// The violation that ended the channel, if any.
    pub(in crate::updates) const fn failure(&self) -> Option<AttemptChannelError> {
        self.failure
    }

    /// Whether the helper proved its identity on this channel.
    pub(in crate::updates) const fn authenticated(&self) -> bool {
        self.authenticated
    }

    /// Whether the helper's result arrived and is still held.
    pub(in crate::updates) const fn has_outcome(&self) -> bool {
        self.outcome.is_some()
    }

    /// The transport's terminal outcome was observed by `poll` or `settled`.
    pub(in crate::updates) const fn is_settled(&self) -> bool {
        self.terminal.is_some()
    }

    /// Ask the helper to stop at its next safe point. It is sent once the helper authenticated.
    pub(in crate::updates) fn request_cancel(&mut self) {
        if self.cancel == Cancel::None {
            self.cancel = Cancel::Wanted;
        }
    }

    /// Drain transport facts and validate frames. A violation is kept and returned once; the
    /// caller then closes the channel and treats the attempt as failed.
    pub(in crate::updates) fn poll(&mut self, now: Moment) -> Result<(), AttemptChannelError> {
        if self.failure.is_none()
            && let Err(cause) = self.drive(now)
        {
            self.failure = Some(cause);
            self.close();
            self.terminal = self.terminal.take().or_else(|| self.session.take_terminal());
            return Err(cause);
        }
        if self.terminal.is_none() {
            self.terminal = self.session.take_terminal();
        }
        Ok(())
    }

    fn drive(&mut self, now: Moment) -> Result<(), AttemptChannelError> {
        for _ in 0..32 {
            let Some(fact) = self.session.drain(NonZeroUsize::MIN).into_iter().next() else {
                break;
            };
            match fact {
                PurposePipeObservation::Prepared => {
                    if self.prepared {
                        return Err(LifecyclePipeError::InvalidMessage.into());
                    }
                    self.prepared = true;
                    self.pins = self.session.take_bootstrap()?;
                }
                PurposePipeObservation::Connected => {
                    if self.process.is_none() {
                        return Err(LifecyclePipeError::NotReady.into());
                    }
                    let operation = self.next()?;
                    let original = self.session.control().original_deadline();
                    self.session.control().read(operation, original)?;
                }
                PurposePipeObservation::Authenticated(peer) => {
                    if peer.key() != PurposeKey::UpdateAttempt(self.channel)
                        || peer.process() != self.process
                        || !self.verification_queued
                        || self.authenticated
                    {
                        return Err(LifecyclePipeError::InvalidMessage.into());
                    }
                    self.authenticated = true;
                    self.read()?;
                }
                PurposePipeObservation::Received { frame, .. } => {
                    if !self.authenticated {
                        let candidate: [u8; 32] = frame
                            .as_bytes()
                            .try_into()
                            .map_err(|_| LifecyclePipeError::InvalidMessage)?;
                        drop(frame);
                        let original = self.session.control().original_deadline();
                        self.session.control().verify_capability(candidate, original)?;
                        self.verification_queued = true;
                        continue;
                    }
                    if self.outcome.is_some() {
                        return Err(AttemptChannelError::AfterResult);
                    }
                    let message = codec::decode(self.channel.attempt(), frame.as_bytes())
                        .map_err(AttemptChannelError::Frame)?;
                    drop(frame);
                    match message {
                        AttemptMessage::Progress { progress } => {
                            self.progress = Some(progress);
                            self.read()?;
                        }
                        AttemptMessage::Finished { result } => self.outcome = Some(result),
                    }
                }
                PurposePipeObservation::WriteRevoked { buffer, .. } => self.buffer = Some(buffer),
                PurposePipeObservation::ReadPending { .. }
                | PurposePipeObservation::WriteIssued { .. }
                | PurposePipeObservation::Written { .. } => {}
            }
        }
        if self.cancel == Cancel::Wanted && self.authenticated && self.outcome.is_none() {
            let operation = self.next()?;
            if self.buffer.is_none() {
                self.buffer = self.session.control().take_write_buffer()?;
            }
            let buffer = self.buffer.take().ok_or(LifecyclePipeError::Busy)?;
            let until =
                now.checked_add(CANCEL_WRITE).map_err(|_| LifecyclePipeError::InvalidDeadline)?;
            match self.session.control().write_cancel_request(operation, until, buffer) {
                Ok(_) => self.cancel = Cancel::Sent,
                Err(rejected) => {
                    self.buffer = Some(rejected.buffer);
                    return Err(rejected.error.into());
                }
            }
        }
        Ok(())
    }

    fn next(&mut self) -> Result<LifecyclePipeOperationId, LifecyclePipeError> {
        self.next_operation =
            self.next_operation.checked_add(1).ok_or(LifecyclePipeError::InvalidOperation)?;
        LifecyclePipeOperationId::new(self.next_operation)
    }

    fn read(&mut self) -> Result<(), LifecyclePipeError> {
        let operation = self.next()?;
        self.session.control().read_passive(operation)
    }

    /// Fence new transport work; retirement is observed through `settled`.
    pub(in crate::updates) fn close(&mut self) {
        // Retirement waits for every charged loan, so return the buffer before fencing.
        drop(self.buffer.take());
        drop(self.pins.take());
        self.session.control().close();
    }

    /// The transport's terminal outcome has been observed.
    pub(in crate::updates) fn settled(&mut self) -> bool {
        if self.terminal.is_none() {
            self.terminal = self.session.take_terminal();
        }
        self.terminal.is_some()
    }
}
