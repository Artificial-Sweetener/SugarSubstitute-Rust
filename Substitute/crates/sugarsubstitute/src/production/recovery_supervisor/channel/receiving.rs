// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt the exact successful receiver output while retaining its original completion lease.
use super::*;
impl Channel {
    pub(super) fn observe_receiver(&mut self) -> Result<()> {
        if self.receiver_terminal.is_some() {
            return Ok(());
        }
        let Some(done) = self.receiver.as_mut().and_then(OwnedJob::try_take_completion) else {
            return Ok(());
        };
        let valid = self.receiver_original == Some((done.scope_id, done.spec))
            && done.memory.bytes() == 4 * 1024 * 1024
            && done.native_handles.is_none();
        let JobCompletion {
            scope_id,
            spec,
            queue_time,
            execution_time,
            outcome,
            memory,
            native_handles,
        } = done;
        let outcome = match outcome {
            JobOutcome::Success(assembly) => {
                self.assembly = Some(assembly);
                JobOutcome::Success(())
            }
            JobOutcome::Failure(cause) => {
                JobOutcome::Failure(terminal::ReceiverCause(Arc::new(cause)))
            }
            JobOutcome::Panicked(cause) => JobOutcome::Panicked(cause),
            JobOutcome::Cancelled => JobOutcome::Cancelled,
            JobOutcome::TimedOut => JobOutcome::TimedOut,
            JobOutcome::Coalesced { replacement } => JobOutcome::Coalesced { replacement },
        };
        self.receiver_terminal = Some(JobCompletion {
            scope_id,
            spec,
            queue_time,
            execution_time,
            outcome,
            memory,
            native_handles,
        });
        self.receiver = None;
        if !valid {
            return Err("Recovery receiver original completion rejected".into());
        }
        terminal::classify(
            &self.receiver_terminal.as_ref().ok_or("Recovery receiver terminal absent")?.outcome,
            self.closing,
        )
    }
    pub(super) fn receive_startup(&mut self) -> Result<()> {
        let Some(frame) = self.received.as_ref() else {
            return Ok(());
        };
        let committed = matches!(
            decode(frame.as_bytes(), RecoveryReceiver::Supervisor)?.command,
            RecoveryCommand::Commit(_)
        );
        self.assembly.as_mut().ok_or("Recovery startup receiver absent")?.receive(frame)?;
        self.received = None;
        if committed {
            let assembly = self.assembly.take().ok_or("Recovery startup receiver disappeared")?;
            match assembly.finish() {
                Ok((startup, sequence)) => {
                    if sequence != self.rx_sequence {
                        return Err("Recovery startup sequence changed".into());
                    }
                    self.startup = Some(startup);
                }
                Err((cause, assembly)) => {
                    self.assembly = Some(assembly);
                    return Err(Box::new(cause));
                }
            }
        }
        Ok(())
    }
}
