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
        publish: impl FnOnce() -> bool,
    ) -> bool {
        let mut state = lock(&self.state);
        let Some(record) = state.jobs.get_mut(&id) else {
            return false;
        };
        record.phase = JobPhase::Terminal(kind);
        let notice =
            CompletionNotification { scope_id: record.scope_id, job_id: id, kind, replacement };
        if publish() {
            record.retiring = false;
            self.mailbox.enqueue(notice)
        } else {
            record.phase = JobPhase::Queued;
            record.retiring = true;
            let job = record.job.clone();
            state.retired.push_back(Retirement::Outcome(job));
            false
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
            let job = record.job.clone();
            self.mailbox.remove(id);
            state.retired.push_back(Retirement::Outcome(job));
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

    #[cfg(any(windows, test))]
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
            let removed = if deferred {
                if let Some(record) = state.jobs.get_mut(&id) {
                    record.acknowledged = true;
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
