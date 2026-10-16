// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve actual typed causes and original guards independently of cleanup requests.
use super::*;
#[derive(Clone, Debug)]
pub(super) struct ReceiverCause(pub(super) Arc<RecoveryStartupError>);
impl std::fmt::Display for ReceiverCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.0.as_ref(), f)
    }
}
impl Error for ReceiverCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.0.as_ref())
    }
}
pub(in crate::production::recovery_supervisor) fn classify<T, E: Error + Clone + 'static>(
    outcome: &JobOutcome<T, E>,
    closing: bool,
) -> Result<()> {
    match outcome {
        JobOutcome::Success(_) => Ok(()),
        JobOutcome::Cancelled if closing => Ok(()),
        JobOutcome::Failure(cause) => Err(Box::new(cause.clone())),
        other => Err(format!("Recovery original owner ended with {:?}", other.kind()).into()),
    }
}
impl Channel {
    /// Native publication occurs only after actual I/O, callback, handle and frame retirement.
    pub(super) fn observe_terminal(&mut self) -> Result<()> {
        if self.pipe_terminal.is_some() {
            return Ok(());
        }
        let Some(done) = self.session.take_terminal() else {
            return Ok(());
        };
        let valid = (done.scope_id, done.spec) == self.pipe_original
            && done.memory.bytes() == MIN_LIFECYCLE_PIPE_BYTES
            && done.native_handles.is_none()
            && match &done.outcome {
                JobOutcome::Success(report) => report.key == self.key,
                _ => true,
            };
        self.pipe_terminal = Some(done);
        if !valid {
            return Err("Recovery pipe original completion rejected".into());
        }
        classify(
            &self.pipe_terminal.as_ref().ok_or("Recovery pipe terminal missing")?.outcome,
            self.closing,
        )?;
        if !self.closing {
            return Err("Authenticated recovery Root transport ended".into());
        }
        Ok(())
    }
}
