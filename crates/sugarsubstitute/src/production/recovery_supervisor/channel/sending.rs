// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode directly into the sole original charged output buffer and observe actual writes.
use super::*;
impl Outgoing {
    fn command(&self) -> (RecoveryCommand<'_>, Sent) {
        match *self {
            Self::StartupAccepted => (RecoveryCommand::StartupAccepted, Sent::StartupAccepted),
            Self::CaptureRegistered { supervisor, handler } => (
                RecoveryCommand::CaptureRegistered { supervisor, handler },
                Sent::CaptureRegistered,
            ),
            Self::DesktopCreated(value) => {
                (RecoveryCommand::DesktopCreated(value), Sent::DesktopCreated)
            }
            Self::DesktopExited { process, status } => {
                (RecoveryCommand::DesktopExited { process, status }, Sent::DesktopExited)
            }
        }
    }
}
impl Channel {
    /// Submit one closed event under its existing admission or original cleanup cutoff.
    /// False preserves caller state when the original output allocation is still occupied.
    /// A true result means queue admission; `take_sent` reports actual complete native write.
    pub(in crate::production::recovery_supervisor) fn try_send(
        &mut self,
        value: &Outgoing,
        until: Moment,
    ) -> Result<bool> {
        if self.closing || !self.hello_written || !self.startup_transferred {
            return Err("Recovery lifecycle sender unavailable".into());
        }
        let (command, sent) = value.command();
        self.send(command, sent, until)
    }
    pub(super) fn hello(&mut self) -> Result<()> {
        if self.tx_sequence != 0 || !self.authenticated {
            return Err("Recovery Hello order rejected".into());
        }
        let bytes = *self.capability.as_ref().ok_or("Recovery Hello capability missing")?.bytes();
        if self.send(RecoveryCommand::Hello(bytes), Sent::Hello, self.original_until())? {
            self.capability = None;
        }
        Ok(())
    }
    fn send(&mut self, command: RecoveryCommand<'_>, sent: Sent, until: Moment) -> Result<bool> {
        if self.write.is_some() || self.sent.is_some() {
            return Ok(false);
        }
        let Some(mut buffer) = self.session.control().take_write_buffer()? else {
            return Ok(false);
        };
        let sequence =
            self.tx_sequence.checked_add(1).ok_or("Recovery write sequence exhausted")?;
        let bytes = encode(
            &RecoveryFrame {
                key: self.key.ok_or("Recovery sender key missing")?,
                sequence,
                command,
            },
            RecoveryReceiver::Coordinator,
            buffer.as_mut_bytes(),
        )?;
        let operation = self.next_operation()?;
        match self.session.control().write(operation, until, buffer, bytes) {
            Ok(admission) => {
                self.write = Some(Write {
                    receipt: receipt::Receipt::new(operation, sequence, bytes),
                    sent,
                    admission,
                    returned: None,
                });
                Ok(true)
            }
            Err(rejected) => {
                // The failed admission returns its original allocation; dropping only returns this loan.
                let cause = rejected.error;
                drop(rejected.buffer);
                Err(Box::new(cause))
            }
        }
    }
}
