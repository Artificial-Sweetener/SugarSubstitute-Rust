// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer authenticated download messages directly to the admitted native storage service.
use super::*;
use std::{sync::mpsc::RecvTimeoutError, time::Duration};
use sugarsubstitute_contracts::execution::JobContext;

/// Sole native download receiver; raw channels, runtime handles and sockets remain private.
/// Keep this port with the command's observed native service, independently of its job result.
pub struct DownloadPort {
    pub(super) input: Receiver<Frame>,
    pub(super) commands: Sender<String>,
    pub(super) budget: memory::Budget,
}
impl DownloadPort {
    /// Take one already queued request without waiting or interpreting native authority.
    pub fn try_receive(&self) -> Option<Frame> {
        let frame = self.input.try_recv().ok();
        if frame.is_some() {
            self.budget.native_taken();
        }
        frame
    }

    /// Wait at most ten milliseconds on an admitted blocking worker, checking its own controls.
    /// # Errors
    /// Reports actual transport closure, cancellation and expired job/scope deadlines.
    pub fn receive(&self, context: &JobContext) -> io::Result<Option<Frame>> {
        self.check(context)?;
        let frame = match self.input.recv_timeout(Duration::from_millis(10)) {
            Ok(frame) => {
                self.budget.native_taken();
                Some(frame)
            }
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return Err(io::ErrorKind::BrokenPipe.into()),
        };
        self.check(context)?;
        Ok(frame)
    }

    /// Recheck after a bounded wait so expired input cannot authorize a subsequent storage effect.
    fn check(&self, context: &JobContext) -> io::Result<()> {
        if context.cancellation_requested() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if context.deadline().is_some_and(|deadline| context.now() >= deadline) {
            return Err(io::ErrorKind::TimedOut.into());
        }
        if self.budget.closed() {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        Ok(())
    }

    /// Queue one bounded native reply through the original authenticated connection.
    /// # Errors
    /// Preserves the same original admission, frame validation and explicit overload as host commands.
    pub fn send(&self, reply: String) -> io::Result<()> {
        send(&self.commands, &self.budget, reply)
    }

    /// Observe queued input only; absence never establishes filesystem or process retirement.
    pub fn pending(&self) -> bool {
        self.budget.native_pending()
    }
}
