// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit fixture commands only after actual authentication and source evidence.
use super::{Result, channel::Message, corpus, options::Role, parent::Parent};
use sugarsubstitute_contracts::execution::CompletionBatch;
impl Parent {
    pub(super) fn control(&mut self, batch: &CompletionBatch) -> Result<()> {
        let case = &mut self.cases[self.current];
        let message =
            case.channel.as_mut().map(|channel| channel.activity(batch)).transpose()?.flatten();
        match message {
            Some(Message::Primed)
                if matches!(case.role, Role::Flood | Role::Idle) && !case.primed =>
            {
                case.primed = true
            }
            Some(Message::Done)
                if matches!(case.role, Role::Flood | Role::Forward) && case.go && !case.done =>
            {
                case.done = true
            }
            Some(Message::DiagnosticFailed)
                if case.role == Role::Idle && case.exit_sent && case.diagnostic_error.is_none() =>
            {
                case.diagnostic_error =
                    case.channel.as_ref().and_then(|channel| channel.diagnostic_error);
                if case.diagnostic_error.is_none() {
                    return Err("Actual diagnostic error reply missing".into());
                }
            }
            Some(_) => {
                return Err("Output parent received unexpected/repeated source command".into());
            }
            None => {}
        }
        let Some(channel) = &mut case.channel else { return Ok(()) };
        if self.closing {
            if !channel.terminal {
                channel.close();
            }
            return Ok(());
        }
        let report = case.output.as_ref().ok_or("Output source missing")?.control.snapshot();
        if case.role == Role::Forward
            && channel.ready
            && channel.last_written == Some(Message::Welcome)
            && !case.go
        {
            channel.queue(Message::Go)?;
            case.go = true;
        }
        if case.role == Role::Flood && case.primed && case.guard.is_some() && !case.go {
            channel.queue(Message::Go)?;
            case.go = true;
        }
        if case.role == Role::Idle
            && case.primed
            && case.pending_before_cancel.is_none()
            && report.pending_reads == [true, true]
            && report.pending_read_issues.iter().all(|n| *n > 0)
            && case.exited.is_none()
        {
            case.pending_before_cancel = Some(report);
            case.output.as_ref().ok_or("Idle output missing")?.control.close();
        }
        let flood_complete = case.role == Role::Flood
            && case.done
            && report
                .counters
                .iter()
                .all(|c| c.read_bytes == corpus::FLOOD_BYTES + 1 && c.dropped_bytes > 0);
        if !case.exit_sent
            && (flood_complete
                || case.role == Role::Idle && case.output_terminal.is_some()
                || case.role == Role::Forward && case.done)
        {
            channel.queue(Message::Exit)?;
            case.exit_sent = true;
        }
        if case.exited.is_some() && !channel.terminal && !channel.closing {
            channel.close();
        }
        Ok(())
    }
}
