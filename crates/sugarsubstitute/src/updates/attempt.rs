// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Report one update attempt to the process that launched this helper over its inherited
//! update-attempt channel: live progress, then exactly one result. The only message the parent
//! may send back is a cancel request.
use std::{num::NonZeroUsize, time::Duration};
use sugarsubstitute_contracts::{
    execution::{JobCompletion, JobSpec, Moment, Scope},
    update::{
        Admission,
        attempt::{
            AttemptOutcome, AttemptPhase, AttemptProgress, UpdateReport,
            codec::{self, AttemptMessage, UpdateAttemptChannel},
        },
    },
};
use sugarsubstitute_execution::{
    ExecutionClient, PurposePipeResources,
    windows_purpose_pipe::{
        InheritedLifecycleBootstrap, LifecycleClientCapability, LifecyclePipeDeadline,
        LifecyclePipeError, LifecyclePipeFailure, LifecyclePipeOperationId,
        LifecyclePipeWriteBuffer, Purpose, PurposeKey, PurposePipeObservation, PurposePipeReport,
        PurposePipeSession,
    },
};
use sugarsubstitute_supervision::update_preparation::Progress;

/// Bound of one progress or result write; a parent that stops reading cannot hold the helper.
const WRITE: Duration = Duration::from_secs(10);

/// Where the one capability Hello stands; frames follow only after it is written.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Hello {
    Unsent,
    Issued,
    Written,
}

/// The helper's end of one attempt channel. It owns its pipe session until observed retirement.
pub(super) struct AttemptReporter {
    input: Option<InheritedLifecycleBootstrap>,
    original: LifecyclePipeDeadline,
    session: Option<PurposePipeSession>,
    channel: Option<UpdateAttemptChannel>,
    capability: Option<LifecycleClientCapability>,
    authenticated: bool,
    hello: Hello,
    buffer: Option<LifecyclePipeWriteBuffer>,
    writing: Option<LifecyclePipeOperationId>,
    result_in_flight: bool,
    progress: Option<AttemptProgress>,
    sent: Option<AttemptProgress>,
    result: Option<AttemptOutcome>,
    delivered: bool,
    cancel_requested: bool,
    next_operation: u64,
    failure: Option<LifecyclePipeError>,
    terminal: Option<JobCompletion<PurposePipeReport, LifecyclePipeFailure>>,
}
impl AttemptReporter {
    /// Hold the inherited section inert until `reserve` admits the transport.
    pub(super) fn new(input: InheritedLifecycleBootstrap, original: LifecyclePipeDeadline) -> Self {
        Self {
            input: Some(input),
            original,
            session: None,
            channel: None,
            capability: None,
            authenticated: false,
            hello: Hello::Unsent,
            buffer: None,
            writing: None,
            result_in_flight: false,
            progress: None,
            sent: None,
            result: None,
            delivered: false,
            cancel_requested: false,
            next_operation: 0,
            failure: None,
            terminal: None,
        }
    }

    /// Admit the helper side of the channel through the execution owner and bind the section.
    /// # Errors
    /// Rejects a second reservation and an admission the execution owner refuses.
    pub(super) fn reserve(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let input = self.input.take().ok_or(LifecyclePipeError::AlreadyTransferred)?;
        let permit = client.reserve_purpose_pipe_client(
            scope,
            spec,
            Purpose::UpdateAttempt,
            self.original,
            PurposePipeResources::Standard,
        )?;
        self.session = Some(permit.bind(input).ok_or(LifecyclePipeError::Closed)?);
        Ok(())
    }

    /// Record the latest progress; it replaces progress not yet written, and progress equal to
    /// what was last written is not sent again.
    pub(super) fn progress(&mut self, progress: AttemptProgress) {
        if self.result.is_none()
            && !self.result_in_flight
            && !self.delivered
            && self.sent != Some(progress)
        {
            self.progress = Some(progress);
        }
    }

    /// Whether a result was recorded, whether or not it reached the pipe yet.
    pub(super) const fn has_result(&self) -> bool {
        self.result.is_some() || self.result_in_flight || self.delivered
    }

    /// Nothing more will be written: the result was delivered, the transport failed, or no
    /// result was ever recorded and no write is outstanding.
    pub(super) const fn quiet(&self) -> bool {
        self.delivered
            || self.failure.is_some()
            || (self.result.is_none() && !self.result_in_flight && self.writing.is_none())
    }

    /// Record the attempt's one result. Later results are ignored: the first one is the answer.
    pub(super) fn finish(&mut self, outcome: AttemptOutcome) {
        if self.result.is_none() && !self.result_in_flight && !self.delivered {
            self.progress = None;
            self.result = Some(outcome);
        }
    }

    /// Whether the parent asked this attempt to stop.
    pub(super) const fn cancel_requested(&self) -> bool {
        self.cancel_requested
    }

    /// Drain transport facts, authenticate, then write pending frames one at a time.
    /// # Errors
    /// Returns the first transport or protocol error; the session then needs `close`.
    pub(super) fn poll(&mut self, now: Moment) -> Result<(), LifecyclePipeError> {
        if let Some(cause) = self.failure {
            return Err(cause);
        }
        let outcome = self.drive(now);
        if let Err(cause) = outcome {
            self.failure.get_or_insert(cause);
        }
        if self.terminal.is_none() {
            self.terminal = self.session.as_mut().and_then(PurposePipeSession::take_terminal);
        }
        outcome
    }

    fn drive(&mut self, now: Moment) -> Result<(), LifecyclePipeError> {
        for _ in 0..32 {
            let Some(session) = &mut self.session else { return Ok(()) };
            let Some(fact) = session.drain(NonZeroUsize::MIN).into_iter().next() else { break };
            match fact {
                PurposePipeObservation::Prepared => {
                    let channel = session
                        .control()
                        .prepared_key()
                        .and_then(PurposeKey::update_attempt)
                        .ok_or(LifecyclePipeError::NotReady)?;
                    if self.channel.replace(channel).is_some() {
                        return Err(LifecyclePipeError::InvalidMessage);
                    }
                    self.capability = session.control().take_client_capability()?;
                }
                PurposePipeObservation::Connected => {
                    let capability =
                        self.capability.as_ref().ok_or(LifecyclePipeError::NotReady)?;
                    session.control().verify_capability(
                        *capability.bytes(),
                        session.control().original_deadline(),
                    )?;
                }
                PurposePipeObservation::Authenticated(peer) => {
                    let expected = self.channel.map(PurposeKey::UpdateAttempt);
                    if Some(peer.key()) != expected || self.authenticated {
                        return Err(LifecyclePipeError::InvalidMessage);
                    }
                    self.authenticated = true;
                }
                PurposePipeObservation::Received { frame, .. } => {
                    let channel = self.channel.ok_or(LifecyclePipeError::NotReady)?;
                    codec::decode_cancel(channel.attempt(), frame.as_bytes())
                        .map_err(|_| LifecyclePipeError::InvalidMessage)?;
                    drop(frame);
                    self.cancel_requested = true;
                    self.read()?;
                }
                PurposePipeObservation::Written { operation, .. } => {
                    if self.writing != Some(operation) {
                        return Err(LifecyclePipeError::InvalidOperation);
                    }
                    self.writing = None;
                    if self.hello == Hello::Issued {
                        self.hello = Hello::Written;
                        self.capability = None;
                        self.read()?;
                    } else if self.result_in_flight {
                        self.result_in_flight = false;
                        self.delivered = true;
                    }
                }
                PurposePipeObservation::WriteRevoked { buffer, .. } => {
                    self.buffer = Some(buffer);
                    return Err(LifecyclePipeError::Closed);
                }
                PurposePipeObservation::ReadPending { .. }
                | PurposePipeObservation::WriteIssued { .. } => {}
            }
        }
        if self.authenticated && self.hello == Hello::Unsent {
            let capability = *self.capability.as_ref().ok_or(LifecyclePipeError::NotReady)?.bytes();
            self.write(&capability, self.session_deadline()?)?;
            self.hello = Hello::Issued;
        } else if self.hello == Hello::Written && self.writing.is_none() {
            let message = if let Some(result) = self.result.take() {
                self.result_in_flight = true;
                Some(AttemptMessage::Finished { result })
            } else {
                self.progress.take().map(|progress| {
                    self.sent = Some(progress);
                    AttemptMessage::Progress { progress }
                })
            };
            if let Some(message) = message {
                let channel = self.channel.ok_or(LifecyclePipeError::NotReady)?;
                let bytes = codec::encode(channel.attempt(), &message)
                    .map_err(|_| LifecyclePipeError::InvalidMessage)?;
                let until =
                    now.checked_add(WRITE).map_err(|_| LifecyclePipeError::InvalidDeadline)?;
                self.write(&bytes, until)?;
            }
        }
        Ok(())
    }

    fn session_deadline(&self) -> Result<Moment, LifecyclePipeError> {
        Ok(self.session.as_ref().ok_or(LifecyclePipeError::NotReady)?.control().original_deadline())
    }

    fn next(&mut self) -> Result<LifecyclePipeOperationId, LifecyclePipeError> {
        self.next_operation =
            self.next_operation.checked_add(1).ok_or(LifecyclePipeError::InvalidOperation)?;
        LifecyclePipeOperationId::new(self.next_operation)
    }

    /// Keep one passive read open for the parent's cancel request.
    fn read(&mut self) -> Result<(), LifecyclePipeError> {
        let operation = self.next()?;
        self.session.as_ref().ok_or(LifecyclePipeError::NotReady)?.control().read_passive(operation)
    }

    fn write(&mut self, bytes: &[u8], until: Moment) -> Result<(), LifecyclePipeError> {
        let operation = self.next()?;
        let session = self.session.as_ref().ok_or(LifecyclePipeError::NotReady)?;
        if self.buffer.is_none() {
            self.buffer = session.control().take_write_buffer()?;
        }
        let mut buffer = self.buffer.take().ok_or(LifecyclePipeError::Busy)?;
        let Some(target) = buffer.as_mut_bytes().get_mut(..bytes.len()) else {
            self.buffer = Some(buffer);
            return Err(LifecyclePipeError::InvalidMessage);
        };
        target.copy_from_slice(bytes);
        match session.control().write(operation, until, buffer, bytes.len()) {
            Ok(_) => {
                self.writing = Some(operation);
                Ok(())
            }
            Err(rejected) => {
                self.buffer = Some(rejected.buffer);
                Err(rejected.error)
            }
        }
    }

    /// Fence new transport work and return the retained buffer, whose loan retirement waits for;
    /// retirement is observed through `settled`.
    pub(super) fn close(&mut self) {
        drop(self.buffer.take());
        if let Some(session) = &self.session {
            session.control().close();
        }
    }

    /// The transport's terminal outcome has been observed, or nothing was ever admitted.
    pub(super) fn settled(&self) -> bool {
        self.session.is_none() || self.terminal.is_some()
    }
}

/// The attempt progress a preparation boundary shows; terminal boundaries show none.
pub(super) fn attempt_progress(progress: Progress) -> Option<AttemptProgress> {
    let phase = match progress {
        Progress::Opening => AttemptPhase::Opening,
        Progress::Checking | Progress::Restoring | Progress::Remembering => AttemptPhase::Checking,
        Progress::SelectingEngine | Progress::CheckingEngine => AttemptPhase::CheckingUpdater,
        Progress::RestoringArchive => AttemptPhase::Downloading,
        Progress::Downloading { received, total } => {
            return Some(AttemptProgress {
                phase: AttemptPhase::Downloading,
                bytes: Some((received.min(total), total)),
            });
        }
        Progress::Reopening | Progress::Preparing => AttemptPhase::Preparing,
        Progress::PreparingEngine => AttemptPhase::PreparingUpdater,
        _ => return None,
    };
    Some(AttemptProgress { phase, bytes: None })
}

/// The result a terminal preparation boundary reports. A prepared candidate needs the version
/// of the offer it authenticated; without one the attempt cannot claim it is ready.
pub(super) fn attempt_outcome(
    progress: Progress,
    version: Option<&str>,
    phase: AttemptPhase,
) -> AttemptOutcome {
    match (progress, version) {
        (Progress::Ready, Some(version)) if !version.is_empty() => {
            AttemptOutcome::Prepared { version: version.to_owned() }
        }
        (Progress::AlreadyPreparing, _) => AttemptOutcome::AlreadyPreparing,
        (Progress::Decision(Admission::Current), _) => AttemptOutcome::Current,
        (Progress::Decision(Admission::Installer), _) => AttemptOutcome::NeedsInstaller,
        _ => AttemptOutcome::Failed(UpdateReport::from_error(
            version,
            phase,
            &std::io::Error::other(format!("Update preparation ended at {progress:?}")),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_boundaries_map_to_their_phase_and_downloads_carry_bytes() {
        assert_eq!(
            attempt_progress(Progress::Downloading { received: 5, total: 9 }),
            Some(AttemptProgress { phase: AttemptPhase::Downloading, bytes: Some((5, 9)) })
        );
        assert_eq!(
            attempt_progress(Progress::CheckingEngine).map(|progress| progress.phase),
            Some(AttemptPhase::CheckingUpdater)
        );
        for terminal in [Progress::Ready, Progress::Failed, Progress::Closed, Progress::Retiring] {
            assert_eq!(attempt_progress(terminal), None);
        }
    }

    #[test]
    fn terminal_boundaries_report_what_the_attempt_did() -> Result<(), Box<dyn std::error::Error>> {
        let phase = AttemptPhase::Preparing;
        assert_eq!(
            attempt_outcome(Progress::Ready, Some("2.0.0"), phase),
            AttemptOutcome::Prepared { version: "2.0.0".into() }
        );
        assert_eq!(
            attempt_outcome(Progress::Decision(Admission::Current), None, phase),
            AttemptOutcome::Current
        );
        assert_eq!(
            attempt_outcome(Progress::Decision(Admission::Installer), None, phase),
            AttemptOutcome::NeedsInstaller
        );
        assert_eq!(
            attempt_outcome(Progress::AlreadyPreparing, None, phase),
            AttemptOutcome::AlreadyPreparing
        );
        let AttemptOutcome::Failed(report) = attempt_outcome(Progress::Ready, None, phase) else {
            return Err("A candidate without an authenticated version was reported ready".into());
        };
        assert_eq!(report.phase, AttemptPhase::Preparing);
        Ok(())
    }
}
