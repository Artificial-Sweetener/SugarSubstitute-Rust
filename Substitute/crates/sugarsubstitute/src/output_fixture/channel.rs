// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind a closed fixture exchange to actual native identity and one admitted session.
use super::{Result, options::Role};
use sugarsubstitute_contracts::{
    execution::{CompletionBatch, JobOutcome, TerminalKind},
    lifecycle::{ProcessKey, RunKey},
};
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    LifecycleClientCapability, LifecyclePipeControl, LifecyclePipeObservation,
    LifecyclePipeOperationId, LifecyclePipeSession,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Message {
    Welcome = 1,
    Go = 2,
    Done = 3,
    Exit = 4,
    Primed = 5,
    DiagnosticFailed = 6,
}
pub(super) struct Channel {
    pub(super) session: LifecyclePipeSession,
    pub(super) run: Option<RunKey>,
    pub(super) expected_process: Option<ProcessKey>,
    pub(super) verified: bool,
    pub(super) ready: bool,
    pub(super) terminal: bool,
    pub(super) closing: bool,
    server: bool,
    role: Role,
    capability: Option<LifecycleClientCapability>,
    hello: bool,
    auth_requested: bool,
    write_active: bool,
    expected_write: Option<(LifecyclePipeOperationId, usize)>,
    pending: Option<Message>,
    send_sequence: u64,
    receive_sequence: u64,
    next_operation: u64,
    terminal_notice: Option<TerminalKind>,
    pub(super) last_written: Option<Message>,
    current_write: Option<Message>,
    pub(super) descendant: Option<(u32, u64)>,
    pub(super) diagnostic_error: Option<u32>,
    pub(super) expected_disconnect:
        Option<sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeFailure>,
}
impl Channel {
    pub(super) fn new(session: LifecyclePipeSession, role: Role, run: Option<RunKey>) -> Self {
        Self {
            session,
            server: run.is_some(),
            run,
            role,
            expected_process: None,
            verified: false,
            ready: false,
            terminal: false,
            closing: false,
            capability: None,
            hello: false,
            auth_requested: false,
            write_active: false,
            expected_write: None,
            pending: None,
            send_sequence: 0,
            receive_sequence: 0,
            next_operation: 1,
            terminal_notice: None,
            last_written: None,
            current_write: None,
            descendant: None,
            diagnostic_error: None,
            expected_disconnect: None,
        }
    }
    fn operation(&mut self) -> Result<LifecyclePipeOperationId> {
        let id = LifecyclePipeOperationId::new(self.next_operation)?;
        self.next_operation =
            self.next_operation.checked_add(1).ok_or("Output pipe identity exhausted")?;
        Ok(id)
    }
    fn read(&mut self) -> Result<()> {
        let id = self.operation()?;
        let control = &self.session.control;
        control.read(id, control.original_deadline())?;
        Ok(())
    }
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        let id = self.operation()?;
        let control = &self.session.control;
        let mut buffer = control.take_write_buffer()?.ok_or("Output protocol write buffer busy")?;
        buffer.as_mut_bytes()[..bytes.len()].copy_from_slice(bytes);
        let admission = match control.write(id, control.original_deadline(), buffer, bytes.len()) {
            Ok(admission) => admission,
            Err(rejected) => return Err(Box::new(rejected.error)),
        };
        // This fixture never revokes a write; token Drop preserves the admitted operation.
        drop(admission);
        self.write_active = true;
        self.expected_write = Some((id, bytes.len()));
        Ok(())
    }
    pub(super) fn queue(&mut self, message: Message) -> Result<()> {
        if !self.verified || self.closing || self.pending.is_some() {
            return Err("Output protocol command not admissible".into());
        }
        self.pending = Some(message);
        self.advance()
    }
    fn advance(&mut self) -> Result<()> {
        if !self.write_active
            && let Some(message) = self.pending.take()
        {
            let sequence =
                self.send_sequence.checked_add(1).ok_or("Output wire sequence exhausted")?;
            let wire = encode(
                self.role,
                message,
                sequence,
                if message == Message::Done {
                    self.descendant
                } else if message == Message::DiagnosticFailed {
                    self.diagnostic_error.map(|code| (code, 0))
                } else {
                    None
                },
            );
            self.write_bytes(&wire)?;
            self.send_sequence = sequence;
            self.current_write = Some(message);
        }
        Ok(())
    }
    pub(super) fn activity(&mut self, batch: &CompletionBatch) -> Result<Option<Message>> {
        self.capture(batch)?;
        self.progress()
    }
    pub(super) fn capture(&mut self, batch: &CompletionBatch) -> Result<()> {
        for notice in &batch.notifications {
            if notice.scope_id == self.session.completion.scope_id()
                && notice.job_id == self.session.completion.spec().id
            {
                if self.terminal || self.terminal_notice.is_some_and(|kind| kind != notice.kind) {
                    return Err("Output pipe terminal repeated/changed".into());
                }
                self.terminal_notice = Some(notice.kind);
            }
        }
        Ok(())
    }
    fn progress(&mut self) -> Result<Option<Message>> {
        let mut received = None;
        for _ in 0..8 {
            let Some(observation) = self.session.control.take_observation() else { break };
            match observation {
                LifecyclePipeObservation::Prepared => {
                    let run = self
                        .session
                        .control
                        .prepared_run()
                        .ok_or("Output native bootstrap run missing")?;
                    if self.run.is_some_and(|expected| expected != run) {
                        return Err("Output native run differs".into());
                    }
                    self.run = Some(run);
                    if !self.server {
                        self.capability = self.session.control.take_client_capability()?;
                    }
                }
                LifecyclePipeObservation::Connected => {
                    if self.server {
                        self.read()?;
                    } else {
                        let capability =
                            self.capability.as_ref().ok_or("Output child capability missing")?;
                        self.session.control.verify_capability(
                            *capability.bytes(),
                            self.session.control.original_deadline(),
                        )?;
                    }
                }
                LifecyclePipeObservation::Authenticated(peer) => {
                    if self.verified
                        || Some(peer.run()) != self.run
                        || (self.server
                            && (peer.process() != self.expected_process || !self.auth_requested))
                    {
                        return Err("Output native peer differs from pre-resume enrollment".into());
                    }
                    self.verified = true;
                    if self.server {
                        self.ready = true;
                        self.queue(Message::Welcome)?;
                        self.read()?;
                    } else {
                        let capability =
                            self.capability.take().ok_or("Output Hello capability missing")?;
                        self.write_bytes(capability.bytes())?;
                        self.hello = true;
                        self.read()?;
                    }
                }
                LifecyclePipeObservation::ReadPending { .. } => {}
                LifecyclePipeObservation::WriteIssued { operation, at: _ } => {
                    if !self.write_active
                        || self.expected_write.is_none_or(|(expected, _)| expected != operation)
                    {
                        return Err("Output protocol issued-write identity/order differs".into());
                    }
                }
                LifecyclePipeObservation::WriteRevoked { operation: _, buffer } => {
                    drop(buffer);
                    return Err("Output protocol observed an unexpected write revocation".into());
                }
                LifecyclePipeObservation::Written { bytes, operation } => {
                    if !self.write_active
                        || self.expected_write != Some((operation, bytes as usize))
                    {
                        return Err("Output protocol write was not issued".into());
                    }
                    self.write_active = false;
                    self.expected_write = None;
                    self.last_written = self.current_write.take();
                }
                LifecyclePipeObservation::Received { frame, .. } => {
                    if self.server && !self.hello {
                        let candidate: [u8; 32] = frame
                            .as_bytes()
                            .try_into()
                            .map_err(|_| "Output Hello has wrong size")?;
                        self.session.control.verify_capability(
                            candidate,
                            self.session.control.original_deadline(),
                        )?;
                        self.auth_requested = true;
                        self.hello = true;
                    } else {
                        if !self.verified || received.is_some() {
                            return Err("Output command has no verified peer/overloaded".into());
                        }
                        let (message, sequence, descendant) = decode(self.role, frame.as_bytes())?;
                        if sequence
                            != self
                                .receive_sequence
                                .checked_add(1)
                                .ok_or("Output input sequence exhausted")?
                        {
                            return Err("Output command replay/order differs".into());
                        }
                        self.receive_sequence = sequence;
                        if message == Message::DiagnosticFailed {
                            self.diagnostic_error = descendant.map(|(code, _)| code);
                        } else if descendant.is_some() {
                            self.descendant = descendant;
                        }
                        if message == Message::Welcome && !self.server {
                            self.ready = true;
                        }
                        received = Some(message);
                    }
                    drop(frame);
                    if self.server && received == Some(Message::DiagnosticFailed) {
                        // This authenticated final report settles the fixture exchange;
                        // no additional read is needed after its actual source was adopted.
                        self.close();
                    } else if self.verified && !self.closing {
                        self.read()?;
                    }
                }
            }
        }
        self.advance()?;
        if let Some(kind) = self.terminal_notice
            && !self.terminal
            && let Some(completion) = self.session.completion.try_take_completion()
        {
            self.terminal = true;
            if completion.outcome.kind() != kind
                || completion.scope_id != self.session.completion.scope_id()
                || completion.spec != self.session.completion.spec()
            {
                return Err("Output pipe terminal identity differs".into());
            }
            match completion.outcome {
                JobOutcome::Cancelled if self.closing => {}
                JobOutcome::Failure(error)
                    if self.server
                        && self.last_written == Some(Message::Exit)
                        && error.native_cause().stage() == "pipe_completion"
                        && matches!(error.win32_code(), Some(109 | 233))
                        && !error.contains_owner_panic()
                        && !error.requires_process_shutdown() =>
                {
                    self.expected_disconnect = Some(error);
                }
                JobOutcome::Failure(error) => {
                    tracing::error!(
                        operation = "output_fixture_lifecycle",
                        role = self.role.name(),
                        server = self.server,
                        last_written = ?self.last_written,
                        stage = error.native_cause().stage(),
                        win32_code = error.win32_code(),
                        cause = %error.native_cause(),
                        "Actual output fixture transport failed"
                    );
                    return Err(Box::new(error));
                }
                JobOutcome::Panicked(cause) => return Err(super::failure::panic(cause)),
                _ => return Err("Output pipe unexpectedly stopped".into()),
            }
        }
        Ok(received)
    }
    pub(super) fn close(&mut self) {
        self.closing = true;
        self.pending = None;
        self.session.control.close();
    }
    pub(super) fn control(&self) -> &LifecyclePipeControl {
        &self.session.control
    }
}
pub(super) fn encode(
    role: Role,
    message: Message,
    sequence: u64,
    descendant: Option<(u32, u64)>,
) -> [u8; 26] {
    let mut wire = [0; 26];
    wire[..4].copy_from_slice(b"SSO1");
    wire[4] = message as u8;
    wire[5] = role as u8;
    wire[6..14].copy_from_slice(&sequence.to_le_bytes());
    if let Some((pid, birth)) = descendant {
        wire[14..18].copy_from_slice(&pid.to_le_bytes());
        wire[18..26].copy_from_slice(&birth.to_le_bytes());
    }
    wire
}
type DecodedMessage = (Message, u64, Option<(u32, u64)>);
pub(super) fn decode(role: Role, wire: &[u8]) -> Result<DecodedMessage> {
    if wire.len() != 26 || &wire[..4] != b"SSO1" || wire[5] != role as u8 {
        return Err("Output command role/version/size differs".into());
    }
    let message = match wire[4] {
        1 => Message::Welcome,
        2 => Message::Go,
        3 => Message::Done,
        4 => Message::Exit,
        5 => Message::Primed,
        6 => Message::DiagnosticFailed,
        _ => return Err("Output command opcode differs".into()),
    };
    let sequence = u64::from_le_bytes(wire[6..14].try_into()?);
    if sequence == 0 {
        return Err("Output sequence must be positive".into());
    }
    let pid = u32::from_le_bytes(wire[14..18].try_into()?);
    let birth = u64::from_le_bytes(wire[18..].try_into()?);
    let descendant = if role == Role::Idle && message == Message::DiagnosticFailed {
        if pid == 0 || birth != 0 {
            return Err("Diagnostic native code missing or malformed".into());
        }
        Some((pid, birth))
    } else if role == Role::Forward && message == Message::Done {
        if pid == 0 || birth == 0 {
            return Err("Output descendant identity missing".into());
        }
        Some((pid, birth))
    } else {
        if pid != 0 || birth != 0 {
            return Err("Output unexpected descendant fields".into());
        }
        None
    };
    Ok((message, sequence, descendant))
}
