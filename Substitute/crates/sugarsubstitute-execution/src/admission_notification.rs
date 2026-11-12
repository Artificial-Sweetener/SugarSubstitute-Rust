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
        #[cfg(any(windows, test))]
        self.stop_cancelled_capture(crate::capture_attach::control::CleanupTrigger::ScopeShutdown);
        self.changed();
    }
    /// Request job cleanup before result transfer; scopes own acknowledged native lifetimes.
    pub(crate) fn cancel_job(&self, id: JobId) {
        let state = lock(&self.state);
        if let Some(record) = state.jobs.get(&id)
            && !record.retiring
            && !matches!(record.phase, JobPhase::Terminal(_))
        {
            record.job.cancellation().request();
        }
        #[cfg(any(windows, test))]
        // Acknowledgment transfers the result's lifetime to its actual owner.
        // Scope/owner shutdown still requests cleanup independently of this job.
        let custody = state
            .jobs
            .get(&id)
            .filter(|record| !record.acknowledged)
            .and_then(Record::native_custody_loan);
        drop(state);
        #[cfg(any(windows, test))]
        if let Some(custody) = custody {
            crate::capture_attach::control::request(
                &custody.custody,
                crate::capture_attach::control::CleanupTrigger::Cancelled,
            );
        }
        self.changed();
    }

    #[cfg(any(windows, test))]
    pub(crate) fn stop_cancelled_capture(
        &self,
        trigger: crate::capture_attach::control::CleanupTrigger,
    ) {
        let capture = {
            let state = lock(&self.state);
            state
                .jobs
                .values()
                .filter(|record| {
                    record.job.cancellation().is_requested()
                        || state.scopes.get(&record.scope_id).is_none_or(|scope| scope.closed)
                })
                .filter_map(Record::native_custody_loan)
                .collect::<Vec<_>>()
        };
        for custody in capture {
            crate::capture_attach::control::request(&custody.custody, trigger);
        }
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
                        record.acknowledged
                            && !record.job.cancellation().pending()
                            && !record.held_native()
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
