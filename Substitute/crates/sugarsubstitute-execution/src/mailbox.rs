// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Bound completion metadata and coalesce native wake delivery independently of acknowledgement.

use crate::scope::lock;
use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::execution::{
    CompletionBatch, CompletionNotification, CompletionWake, ExecutionError, JobId,
};

struct Pending {
    notices: VecDeque<CompletionNotification>,
    wake_pending: bool,
    failures: Vec<ExecutionError>,
    omitted_failures: u64,
}

#[cfg(test)]
#[path = "mailbox_tests.rs"]
mod tests;

pub(crate) struct Mailbox {
    pending: Mutex<Pending>,
    wake: Arc<dyn CompletionWake>,
    pub(crate) notify: Arc<tokio::sync::Notify>,
    suppressed_thread: Mutex<Option<std::thread::ThreadId>>,
    deferred_wake: std::sync::atomic::AtomicBool,
}

impl Mailbox {
    pub(crate) fn new(wake: Arc<dyn CompletionWake>) -> Self {
        Self {
            pending: Mutex::new(Pending {
                notices: VecDeque::with_capacity(560),
                wake_pending: false,
                failures: Vec::with_capacity(560),
                omitted_failures: 0,
            }),
            wake,
            notify: Arc::new(tokio::sync::Notify::new()),
            suppressed_thread: Mutex::new(None),
            deferred_wake: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub(crate) fn enqueue(&self, notice: CompletionNotification) -> bool {
        let mut pending = lock(&self.pending);
        pending.notices.push_back(notice);
        let should_wake = !pending.wake_pending;
        pending.wake_pending = true;
        should_wake
    }

    /// Coalesce admission-capacity activity without inventing terminal metadata.
    pub(crate) fn signal_capacity(&self) -> bool {
        let mut pending = lock(&self.pending);
        let should_wake = !pending.wake_pending;
        pending.wake_pending = true;
        should_wake
    }

    pub(crate) fn deliver_wake(&self) {
        if lock(&self.suppressed_thread).is_some_and(|thread| thread == std::thread::current().id())
        {
            self.deferred_wake.store(true, std::sync::atomic::Ordering::Release);
            self.notify.notify_one();
            return;
        }
        let result = match catch_unwind(AssertUnwindSafe(|| self.wake.wake())) {
            Ok(result) => result,
            Err(payload) => Err(ExecutionError::new(
                sugarsubstitute_contracts::execution::Operation::NativeLifecycle,
                Box::new(std::io::Error::other(
                    crate::task::panic_cause(payload).as_str().to_owned(),
                )),
            )),
        };
        if let Err(error) = result {
            tracing::error!(operation = ?error.operation(), "Completion wake failed");
            lock(&self.pending).wake_pending = false;
            self.record_failure(error);
        }
    }
    #[cfg(any(windows, test))]
    pub(crate) fn suppress_current_thread(&self) {
        *lock(&self.suppressed_thread) = Some(std::thread::current().id());
    }
    pub(crate) fn flush_deferred_wake(&self) {
        if self.deferred_wake.swap(false, std::sync::atomic::Ordering::AcqRel) {
            self.deliver_wake();
        }
    }

    pub(crate) fn record_failure(&self, error: ExecutionError) {
        let mut pending = lock(&self.pending);
        if pending.failures.len() < 560 {
            pending.failures.push(error);
        } else {
            pending.omitted_failures = pending.omitted_failures.saturating_add(1);
        }
    }

    pub(crate) fn take_failure(&self) -> Option<ExecutionError> {
        let mut pending = lock(&self.pending);
        Self::take_pending_failure(&mut pending)
    }

    /// Preserve original causes and omitted counts while the caller holds the pending mutex.
    fn take_pending_failure(pending: &mut Pending) -> Option<ExecutionError> {
        if pending.failures.is_empty() {
            return None;
        }
        let mut causes = std::mem::take(&mut pending.failures);
        if pending.omitted_failures != 0 {
            causes.push(crate::runtime::error(std::io::Error::other(format!(
                "{} further execution notification failures omitted",
                pending.omitted_failures
            ))));
            pending.omitted_failures = 0;
        }
        Some(crate::runtime::aggregate_errors(causes))
    }

    pub(crate) fn remove(&self, id: JobId) {
        lock(&self.pending).notices.retain(|notice| notice.job_id != id);
    }

    pub(crate) fn drain(&self, maximum: NonZeroUsize) -> Result<CompletionBatch, ExecutionError> {
        let (result, should_wake) = {
            let mut pending = lock(&self.pending);
            let result = if let Some(error) = Self::take_pending_failure(&mut pending) {
                // Observing the fatal wake acknowledges it without consuming terminal facts.
                Err(error)
            } else {
                let count = maximum.get().min(32).min(pending.notices.len());
                let notifications = pending.notices.drain(..count).collect();
                Ok(CompletionBatch { notifications, remaining: pending.notices.len() })
            };
            let remaining = pending.notices.len();
            pending.wake_pending = remaining != 0;
            (result, remaining != 0)
        };
        if should_wake {
            self.deliver_wake();
        }
        result
    }
}
