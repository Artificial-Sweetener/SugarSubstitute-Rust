// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume exact parent-event terminals while retaining registry cleanup independently.
use super::*;
use std::fmt;
use sugarsubstitute_contracts::execution::{JobOutcome, PanicCause};

/// Preserve the bounded original panic payload behind redacted diagnostics.
struct ParentPanic(PanicCause);
impl fmt::Debug for ParentPanic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParentPanic").field("retained_utf8_bytes", &self.0.as_str().len()).finish()
    }
}
impl fmt::Display for ParentPanic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Supervised parent event owner panic")
    }
}
impl Error for ParentPanic {}

impl Fixture {
    /// Rearm only the family whose actual creation/root/member completion arrived.
    pub(super) fn drain(&mut self, max: NonZeroUsize) -> Result<bool> {
        let Some(client) = &self.client else { return Ok(false) };
        let batch = client.drain_completions(max)?;
        self.registry.consume(&batch)?;
        if let Some((index, id)) = self.cleanup_busy
            && let Some(notice) = batch.notifications.iter().find(|notice| notice.job_id == id)
        {
            self.cleanup_busy = None;
            if let Some(key) = self.process(index) {
                let snapshot = self.registry.snapshot(key)?;
                self.cleanup[index].inventory_completed(
                    notice.kind,
                    snapshot.member_watch_required,
                    snapshot.creation_pending || snapshot.cleanup_pending,
                    snapshot.settled,
                );
            }
        }
        let progress = !batch.notifications.is_empty();
        for index in 0..=HELD {
            if self.cleanup[index].stop.is_none() {
                continue;
            }
            let (creation, root) = if index == SENTINEL {
                (self.sentinel_creation, self.sentinel_watch)
            } else {
                (self.cases[index].creation_job, self.cases[index].evidence.root_job)
            };
            let member = self.cleanup[index].member_watch;
            for notice in &batch.notifications {
                if Some(notice.job_id) == creation || Some(notice.job_id) == root {
                    self.cleanup[index].requested = true;
                }
                if Some(notice.job_id) == member {
                    self.cleanup[index].member_completed(notice.kind);
                    self.cleanup[index].member_watch = None;
                }
            }
        }
        for case in &mut self.cases {
            if let Some(session) = &case.session {
                for notice in &batch.notifications {
                    if (notice.scope_id, notice.job_id)
                        == (session.completion.scope_id(), session.completion.spec().id)
                    {
                        case.notice = Some(notice.kind);
                    }
                }
            }
        }
        Ok(progress)
    }
    /// Consume the exact typed terminal after actual native event retirement.
    pub(super) fn event_terminal(&mut self, index: usize) -> Result<bool> {
        let case = &mut self.cases[index];
        let Some(kind) = case.notice else { return Ok(false) };
        let Some(session) = &mut case.session else { return Ok(false) };
        let expected = (session.completion.scope_id(), session.completion.spec());
        let Some(completion) = session.completion.try_take_completion() else { return Ok(false) };
        case.session = None;
        case.notice = None;
        if completion.scope_id != expected.0
            || completion.spec != expected.1
            || completion.outcome.kind() != kind
        {
            return Err("Parent event terminal identity differs from admitted session".into());
        }
        match completion.outcome {
            JobOutcome::Success(report) => case.evidence.report(report)?,
            JobOutcome::Failure(error) => return Err(Box::new(error)),
            JobOutcome::Panicked(cause) => return Err(Box::new(ParentPanic(cause))),
            JobOutcome::Cancelled | JobOutcome::TimedOut | JobOutcome::Coalesced { .. } => {
                return Err(
                    "Parent event session retired without successful explicit Release".into()
                );
            }
        }
        Ok(true)
    }
}
