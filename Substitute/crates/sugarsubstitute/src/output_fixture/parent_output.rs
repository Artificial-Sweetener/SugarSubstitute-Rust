// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer exact charged source guards and preserve metadata across fallible validation.
use super::{Result, evidence::Case, options::Role};
use sugarsubstitute_contracts::execution::{CompletionBatch, JobOutcome, TerminalKind};
use sugarsubstitute_execution::windows_output::{
    MAX_WINDOWS_OUTPUT_CHUNKS, MAX_WINDOWS_OUTPUT_QUEUED_BYTES, WindowsOutputError,
};
impl Case {
    pub(super) fn capture(&mut self, batch: &CompletionBatch) -> Result<()> {
        for notice in &batch.notifications {
            if self.output.as_ref().is_some_and(|s| {
                s.completion.scope_id() == notice.scope_id
                    && s.completion.spec().id == notice.job_id
            }) {
                self.output_notice = Some(notice.kind);
            }
            if self
                .store_job
                .as_ref()
                .is_some_and(|s| s.scope_id() == notice.scope_id && s.spec().id == notice.job_id)
            {
                self.store_notice = Some(notice.kind);
            }
            if self.cleanup_job == Some(notice.job_id) {
                self.cleanup_job = None;
                self.cleanup_success |= notice.kind == TerminalKind::Success;
            }
            if self.member_job == Some(notice.job_id) {
                self.member_job = None;
                self.member_success |= notice.kind == TerminalKind::Success;
            }
        }
        if let Some(channel) = &mut self.channel {
            channel.capture(batch)?;
        }
        Ok(())
    }
    pub(super) fn consume_jobs(&mut self, closing: bool) -> Result<()> {
        if let Some(kind) = self.store_notice
            && let Some(job) = &mut self.store_job
            && let Some(completion) = job.try_take_completion()
        {
            let expected = (job.scope_id(), job.spec());
            self.store_job = None;
            self.store_notice = None;
            if completion.scope_id != expected.0
                || completion.spec != expected.1
                || completion.outcome.kind() != kind
            {
                return Err("Output transcript initialization terminal differs".into());
            }
            match completion.outcome {
                JobOutcome::Success(store) => self.store = Some(store),
                JobOutcome::Failure(error) => return Err(Box::new(error)),
                JobOutcome::Panicked(cause) => return Err(super::failure::panic(cause)),
                _ if closing => {}
                _ => return Err("Output store did not initialize".into()),
            }
        }
        if let Some(kind) = self.output_notice
            && self.output_terminal.is_none()
            && let Some(session) = &mut self.output
            && let Some(completion) = session.completion.try_take_completion()
        {
            self.output_terminal = Some(completion.outcome.kind());
            self.output_report = Some(session.control.snapshot());
            if completion.scope_id != session.completion.scope_id()
                || completion.spec != session.completion.spec()
                || completion.outcome.kind() != kind
            {
                return Err("Output actual terminal identity differs".into());
            }
            match completion.outcome {
                JobOutcome::Success(report)
                    if self.role != Role::Idle
                        && report.identity == self.output_identity()
                        && report.retired => {}
                JobOutcome::Cancelled if closing || self.pending_before_cancel.is_some() => {}
                JobOutcome::Failure(error) => return Err(Box::new(error)),
                JobOutcome::Panicked(cause) => return Err(super::failure::panic(cause)),
                _ => return Err("Output terminal class differs from case".into()),
            }
        }
        Ok(())
    }
    pub(super) fn drain_output(&mut self, closing: bool) -> Result<bool> {
        let Some(session) = &self.output else { return Ok(false) };
        let report = session.control.snapshot();
        if report.identity != self.output_identity()
            || report.queued_chunks as usize > MAX_WINDOWS_OUTPUT_CHUNKS
            || report.queued_bytes > MAX_WINDOWS_OUTPUT_QUEUED_BYTES as u64
        {
            return Err("Output source resource/capacity differs".into());
        }
        if self.store.is_none() {
            return Ok(false);
        }
        if self.role == Role::Flood && !closing && !self.exit_sent {
            if self.guard.is_none() {
                self.guard = session.control.take_batch()?;
            }
            if self.guard.is_some() {
                if !matches!(session.control.take_batch(), Err(WindowsOutputError::LoanBusy)) {
                    return Err("Held output loan did not reject second borrow".into());
                }
                self.guard_busy = true;
            }
            return Ok(false);
        }
        let guard = if let Some(guard) = self.guard.take() {
            Some(guard)
        } else {
            session.control.take_batch()?
        };
        // No guard means all presently available bounded data was consumed. A live
        // source remains eligible for later actual output/callback wake.
        if let Some(guard) = guard {
            if !closing {
                let store = self.store.as_mut().ok_or("Output store missing")?;
                if self.role.authenticated() {
                    let batch =
                        guard.desktop_batch().ok_or("Desktop output guard has no payload")?;
                    if batch.run() != self.run {
                        return Err("Foreign desktop output run".into());
                    }
                    store.apply_desktop(self.role, batch)?;
                } else {
                    let batch = guard.batch().ok_or("Managed output guard has no payload")?;
                    store.apply(self.role, batch)?;
                }
            }
            drop(guard);
            return Ok(true);
        }
        if self.output_terminal.is_some() && !self.drained {
            let store = self.store.as_ref().ok_or("Output store missing")?;
            if !closing {
                store.verify(self.role, report)?;
                let expected = if self.role == Role::Idle { [false, false] } else { [true, true] };
                if store.eof != expected
                    || report.eof != expected
                    || report.pending_reads != [false, false]
                    || !report.retired
                {
                    return Err("Actual output EOF/retirement differs".into());
                }
            }
            self.diagnostics = Some(store.transcript.diagnostics());
            self.sequences = store.sequences;
            self.delivered = store.delivered;
            self.gaps = store.gaps;
            self.output_report = Some(report);
            self.store = None;
            self.drained = true;
        }
        Ok(false)
    }
}
