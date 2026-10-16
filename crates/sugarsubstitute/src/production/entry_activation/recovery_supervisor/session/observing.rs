// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate transport order and exact terminal correlation without inventing peer authority.
use super::framing::{Role, Stage};
use super::*;
impl State {
    #[expect(
        clippy::result_large_err,
        reason = "Retain the complete original store cause inline within the funded recovery session."
    )]
    pub(super) fn observe(
        &mut self,
        observation: RecoveryPipeObservation,
    ) -> Result<(), SessionCause> {
        match observation {
            RecoveryPipeObservation::Prepared => {
                if self.prepared {
                    return Err(SessionCause::Protocol(SessionCode::Order));
                }
                self.prepared = true;
            }
            RecoveryPipeObservation::Connected => {
                if self.connected {
                    return Err(SessionCause::Protocol(SessionCode::Order));
                }
                self.connected = true;
            }
            RecoveryPipeObservation::Authenticated(peer) => {
                if !self.hello
                    || self.peer.is_some()
                    || peer.key() != self.grant.key()
                    || peer.process() != Some(self.carrier.process())
                    || peer.process_id() == 0
                    || peer.creation_time_100ns() == 0
                {
                    return Err(SessionCause::Protocol(SessionCode::Binding));
                }
                self.peer = Some(peer);
            }
            RecoveryPipeObservation::ReadPending { operation } => {
                if self.read != Some(operation) {
                    return Err(SessionCause::Protocol(SessionCode::Binding));
                }
            }
            RecoveryPipeObservation::Received { operation, frame } => {
                if self.read != Some(operation) {
                    return Err(SessionCause::Protocol(SessionCode::Binding));
                }
                self.read = None;
                let frame = decode(frame.as_bytes(), RecoveryReceiver::Coordinator)
                    .map_err(SessionCause::Wire)?;
                if frame.key != self.grant.key()
                    || frame.sequence
                        != self
                            .sequence
                            .checked_add(1)
                            .ok_or(SessionCause::Protocol(SessionCode::Exhausted))?
                {
                    return Err(SessionCause::Protocol(SessionCode::Binding));
                }
                let retired = matches!(&frame.command, RecoveryCommand::Retired);
                self.command(frame.command)?;
                self.sequence = frame.sequence;
                if retired {
                    tracing::info!(operation="recovery_supervisor_retired_received", key=?self.grant.key(),
                        sequence=frame.sequence, native_operation=?operation,
                        stop_written=self.closing.as_ref().is_some_and(|close| close.sent),
                        "Exact authenticated recovery Retired command accepted; receive admission ended");
                }
            }
            RecoveryPipeObservation::WriteIssued { operation, .. } => {
                self.plan.issued(operation)?
            }
            RecoveryPipeObservation::WriteRevoked { operation, buffer: _ } => {
                self.plan.revoked(operation)?;
                self.admission = None;
                if self.closing.is_none() {
                    return Err(SessionCause::Protocol(SessionCode::Revoked));
                }
            }
            RecoveryPipeObservation::Written { operation, bytes } => {
                match self.plan.complete(operation, bytes)? {
                    Role::Enrolled => self.enrolled_reply = true,
                    Role::Stop => {
                        if let Some(close) = &mut self.closing {
                            close.sent = true;
                            tracing::info!(operation="recovery_supervisor_stop_written", key=?self.grant.key(),
                                native_operation=?operation, bytes, original_until=?close.until,
                                "Original recovery Stop native write completion consumed");
                        }
                    }
                    _ => {}
                }
                self.admission = None;
            }
        }
        Ok(())
    }
    #[expect(
        clippy::result_large_err,
        reason = "Retain the complete original store cause inline within the funded recovery session."
    )]
    fn command(&mut self, command: RecoveryCommand<'_>) -> Result<(), SessionCause> {
        if let RecoveryCommand::Hello(capability) = command {
            if self.hello || self.peer.is_some() || !self.connected || self.closing.is_some() {
                return Err(SessionCause::Protocol(SessionCode::Order));
            }
            self.pipe
                .control()
                .verify_capability(capability, self.grant.original_budget().until())
                .map_err(SessionCause::Pipe)?;
            self.hello = true;
            return Ok(());
        }
        if self.peer.is_none() {
            return Err(SessionCause::Protocol(SessionCode::Order));
        }
        match command {
            RecoveryCommand::StartupAccepted => {
                if self.plan.stage != Stage::Active
                    || self.startup_accepted
                    || self.closing.is_some()
                {
                    return Err(SessionCause::Protocol(SessionCode::Order));
                }
                self.startup_accepted = true;
            }
            RecoveryCommand::CaptureRegistered { supervisor, handler } => {
                let peer = self.peer.ok_or(SessionCause::Protocol(SessionCode::Binding))?;
                if !self.startup_accepted
                    || self.handler_advisory.is_some()
                    || self.closing.is_some()
                    || supervisor.pid() != peer.process_id()
                    || supervisor.birth() != peer.creation_time_100ns()
                    || supervisor.pid() == handler.pid()
                {
                    return Err(SessionCause::Protocol(SessionCode::Binding));
                }
                self.handler_advisory = Some(handler);
            }
            RecoveryCommand::DesktopCreated(desktop) => {
                if !self.startup_accepted
                    || self.handler_advisory.is_none()
                    || self.desktop_advisory.is_some()
                    || self.handler_advisory.is_some_and(|handler| handler.pid() == desktop.pid())
                    || self.closing.is_some()
                    || self.peer.is_some_and(|peer| desktop.pid() == peer.process_id())
                {
                    return Err(SessionCause::Protocol(SessionCode::Binding));
                }
                self.desktop_advisory = Some(desktop);
            }
            RecoveryCommand::DesktopExited { process, status } => {
                if self.desktop_exit_advisory.is_some()
                    || Some(process) != self.desktop_advisory
                    || !self.enrolled_reply
                {
                    return Err(SessionCause::Protocol(SessionCode::Binding));
                }
                self.desktop_exit_advisory = Some((process, status));
            }
            RecoveryCommand::Retired => {
                self.subordinate_retired
                    .receive(self.closing.is_some())
                    .map_err(SessionCause::Protocol)?;
            }
            RecoveryCommand::Failed(code) => return Err(SessionCause::Remote(code)),
            _ => return Err(SessionCause::Protocol(SessionCode::Order)),
        }
        Ok(())
    }
    pub(super) fn observe_pipe_terminal(&mut self) {
        if self.pipe_terminal.is_some() {
            return;
        }
        let Some(done) = self.pipe.take_terminal() else {
            return;
        };
        let exact = done.scope_id == self.pipe_scope && done.spec == self.pipe_spec;
        let cause = if !exact {
            Some(SessionCause::Protocol(SessionCode::Binding))
        } else {
            match &done.outcome {
                JobOutcome::Success(report)
                    if report.key == Some(self.grant.key()) && self.closing.is_some() =>
                {
                    None
                }
                JobOutcome::Failure(cause) => Some(SessionCause::Transport(cause.clone())),
                JobOutcome::Panicked(cause) => {
                    Some(SessionCause::Panic(SessionPanic(cause.clone())))
                }
                JobOutcome::Cancelled | JobOutcome::TimedOut if self.closing.is_some() => None,
                _ => Some(SessionCause::Protocol(SessionCode::Terminal)),
            }
        };
        // The actual complete native failure/panic stays owned, even if a categorical cause is reported.
        self.pipe_terminal = Some(done);
        if let Some(cause) = cause {
            self.record(cause);
        }
    }
    pub(super) fn observe_watches(&mut self) {
        let shared = self.work.clone();
        let mut work = match shared.try_lock() {
            Ok(work) => work,
            Err(TryLockError::WouldBlock) => return,
            Err(TryLockError::Poisoned(poison)) => {
                if !self.worker_reported {
                    self.worker_reported = true;
                    self.record(SessionCause::Protocol(SessionCode::Worker));
                }
                poison.into_inner()
            }
        };
        let mut failures: [Option<SessionCause>; 2] = [None, None];
        if !self.watch_reported[0]
            && let Some(handler) = &mut work.handler
            && let Err(cause) = handler.observe_terminal()
        {
            self.watch_reported[0] = true;
            failures[0] = Some(SessionCause::Registry(cause));
        }
        if !self.watch_reported[1]
            && let Some(desktop) = &mut work.desktop
            && let Err(cause) = desktop.observe_terminal()
        {
            self.watch_reported[1] = true;
            failures[1] = Some(SessionCause::Registry(cause));
        }
        for handler in [true, false] {
            let (job, terminal) = if handler {
                let Work { cancelled_handler, handler_terminal, .. } = &mut *work;
                (cancelled_handler, handler_terminal)
            } else {
                let Work { cancelled_ui, ui_terminal, .. } = &mut *work;
                (cancelled_ui, ui_terminal)
            };
            if terminal.is_none()
                && let Some(job) = job
            {
                *terminal = job.try_take_completion();
            }
            let index = if handler { 0 } else { 1 };
            if !self.watch_reported[index]
                && let Some(done) = terminal
            {
                let original = match &done.outcome {
                    JobOutcome::Failure(cause) => Some(SessionCause::Native(cause.clone())),
                    JobOutcome::Panicked(cause) => {
                        Some(SessionCause::Panic(SessionPanic(cause.clone())))
                    }
                    JobOutcome::Coalesced { .. } => {
                        Some(SessionCause::Protocol(SessionCode::Terminal))
                    }
                    _ => None,
                };
                if let Some(cause) = original {
                    self.watch_reported[index] = true;
                    failures[index] = Some(cause);
                }
            }
        }
        let mismatch = self
            .desktop_exit_advisory
            .zip(work.desktop.as_ref().and_then(RecoveryEnrolledDesktop::actual_exit))
            .is_some_and(|((process, status), exit)| {
                process.pid() != exit.process_id
                    || process.birth() != exit.creation_time_100ns
                    || status != exit.exit_code
            });
        drop(work);
        for cause in failures.into_iter().flatten() {
            self.record(cause);
        }
        if mismatch && !self.exit_mismatch {
            self.exit_mismatch = true;
            self.record(SessionCause::Protocol(SessionCode::Binding));
        }
    }
}

impl State {
    pub(super) fn owner_failed(&self) -> bool {
        let pipe = self.pipe_terminal.as_ref().is_some_and(|done| match &done.outcome {
            JobOutcome::Panicked(_) => true,
            JobOutcome::Failure(cause) => {
                cause.requires_process_shutdown() || cause.contains_owner_panic()
            }
            _ => false,
        });
        if pipe
            || self
                .job_terminals
                .iter()
                .flatten()
                .any(|done| matches!(&done.outcome, JobOutcome::Panicked(_)))
        {
            return true;
        }
        let Ok(work) = self.work.try_lock() else {
            return self.worker_reported;
        };
        work.failure.iter().flatten().any(RegistryError::contains_owner_panic)
            || work
                .desktop
                .as_ref()
                .and_then(RecoveryEnrolledDesktop::terminal)
                .is_some_and(watch_panic)
            || work
                .handler
                .as_ref()
                .and_then(RecoveryHandlerWatch::terminal)
                .is_some_and(watch_panic)
            || work.ui_terminal.as_ref().is_some_and(watch_panic)
            || work.handler_terminal.as_ref().is_some_and(watch_panic)
    }
}
fn watch_panic(done: &JobCompletion<ProcessExit, NativeObservationError>) -> bool {
    matches!(&done.outcome, JobOutcome::Panicked(_))
        || matches!(&done.outcome,JobOutcome::Failure(cause) if cause.contains_owner_panic())
}
