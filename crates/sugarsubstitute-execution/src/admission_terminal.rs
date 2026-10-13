// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain terminal credits until typed transfer or supervised result destruction.

use super::*;

impl Core {
    pub(crate) fn terminal(
        &self,
        id: JobId,
        kind: TerminalKind,
        replacement: Option<JobId>,
        validate: impl FnOnce(&mut State) -> Result<(), AdmissionError>,
        publish: impl FnOnce() -> bool,
    ) -> Result<bool, AdmissionError> {
        let mut state = lock(&self.state);
        validate(&mut state)?;
        let Some(record) = state.jobs.get_mut(&id) else {
            return Ok(false);
        };
        if record.terminal_published {
            return Ok(false);
        }
        record.terminal_published = true;
        record.phase = JobPhase::Terminal(kind);
        let notice =
            CompletionNotification { scope_id: record.scope_id, job_id: id, kind, replacement };
        if publish() {
            record.retiring = false;
            Ok(self.mailbox.enqueue(notice))
        } else {
            record.phase = JobPhase::Queued;
            record.retiring = true;
            if !crate::entry_preparation::slot::defer_disposal(record.preparation.as_ref()) {
                let job = record.job.clone();
                state.retired.push_back(Retirement::Outcome(job));
            }
            Ok(false)
        }
    }

    pub(crate) fn dispose_terminal(&self, id: JobId) {
        let mut state = lock(&self.state);
        if let Some(record) = state.jobs.get_mut(&id)
            && matches!(record.phase, JobPhase::Terminal(_))
            && !record.retiring
        {
            record.retiring = true;
            record.phase = JobPhase::Queued;
            self.mailbox.remove(id);
            if !crate::entry_preparation::slot::defer_disposal(record.preparation.as_ref()) {
                let job = record.job.clone();
                state.retired.push_back(Retirement::Outcome(job));
            }
        }
        drop(state);
        self.changed();
    }

    pub(crate) fn abandon_all(&self) {
        let jobs: Vec<_> =
            lock(&self.state).jobs.values().map(|record| record.job.clone()).collect();
        for job in jobs {
            job.abandon();
        }
    }

    pub(crate) fn retire_native_custody(&self, id: JobId) {
        let acknowledged = {
            let state = lock(&self.state);
            state.jobs.get(&id).is_some_and(|record| record.acknowledged && !record.held_native())
        };
        if acknowledged {
            self.acknowledge(id);
        }
        self.changed();
    }

    pub(crate) fn acknowledge(&self, id: JobId) {
        let (removed, wake) = {
            let mut state = lock(&self.state);
            self.mailbox.remove(id);
            let deferred = state
                .jobs
                .get(&id)
                .is_some_and(|record| record.job.cancellation().pending() || record.held_native());
            #[cfg(any(windows, test))]
            let process_retained =
                state.jobs.get(&id).is_some_and(|record| record.process_capture.is_some());
            #[cfg(not(any(windows, test)))]
            let process_retained = false;
            let removed = if deferred || process_retained {
                if let Some(record) = state.jobs.get_mut(&id) {
                    record.acknowledged = true;
                    if process_retained && !deferred {
                        // Actual result disposal/transfer has acknowledged the original
                        // historical Success. No terminal notification is republished.
                        record.phase = JobPhase::Terminal(TerminalKind::Success);
                        record.retiring = false;
                    }
                }
                None
            } else {
                state.jobs.remove(&id)
            };
            Self::retire_scopes(&mut state);
            let wake = self.capacity_ready(&mut state);
            (removed, wake)
        };
        drop(removed);
        self.deliver_capacity_wake(wake);
        self.changed();
    }
}
