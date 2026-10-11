// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Deliver cancellation callbacks on owned async workers after atomic control requests.

use super::*;
use sugarsubstitute_contracts::execution::{ExecutionError, Operation};

impl Core {
    pub(crate) fn cancel_scope(&self, id: ScopeId) {
        if let Some(scope) = lock(&self.state).scopes.get(&id) {
            scope.cancel.request();
        }
        self.changed();
    }
    pub(crate) fn cancel_job(&self, id: JobId) {
        let state = lock(&self.state);
        if let Some(record) = state.jobs.get(&id)
            && !record.retiring
            && !matches!(record.phase, JobPhase::Terminal(_))
        {
            record.job.cancellation().request();
        }
        drop(state);
        self.changed();
    }

    pub(crate) fn flush_notifications(&self) {
        let cancellations = {
            let mut state = lock(&self.state);
            let mut cancellations = Vec::with_capacity(756);
            for scope in state.scopes.values() {
                if scope.cancel.take_notification() {
                    cancellations.push(scope.cancel.clone());
                }
            }
            for record in state.jobs.values() {
                let cancellation = record.job.cancellation();
                if cancellation.take_notification() {
                    cancellations.push(cancellation);
                }
            }
            state.notifications_inflight += cancellations.len();
            cancellations
        };
        if cancellations.is_empty() {
            return;
        }
        for cancellation in cancellations {
            if let Err(cause) = cancellation.flush() {
                self.mailbox.record_failure(ExecutionError::new(
                    Operation::Infrastructure,
                    Box::new(std::io::Error::other(cause.as_str().to_owned())),
                ));
                self.mailbox.deliver_wake();
            }
            let acknowledged = {
                let mut state = lock(&self.state);
                state.notifications_inflight -= 1;
                Self::retire_scopes(&mut state);
                state
                    .jobs
                    .iter()
                    .filter(|(_, record)| {
                        record.acknowledged && !record.job.cancellation().pending()
                    })
                    .map(|(id, _)| *id)
                    .collect::<Vec<_>>()
            };
            for id in acknowledged {
                self.acknowledge(id);
            }
        }
        self.changed();
    }
}
