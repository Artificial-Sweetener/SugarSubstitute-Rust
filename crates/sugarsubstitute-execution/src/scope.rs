// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own scope ancestry, cancellation and monotonic lifetime boundaries.

use crate::admission::Core;
use std::sync::{
    Arc, Mutex, MutexGuard, Weak,
    atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_contracts::execution::{
    CancelControl, ExecutionId, Moment, ScopeControl, ScopeId, ScopeSnapshot, ScopeSpec, WaitFuture,
};
use tokio::sync::Notify;

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(error) => {
            tracing::error!(operation = "execution_lock", "Recovering poisoned owned state");
            error.into_inner()
        }
    }
}

pub(crate) struct Cancellation {
    requested: Arc<AtomicBool>,
    notify: Arc<Notify>,
    parent: Option<Arc<Cancellation>>,
    core: Weak<Core>,
    pending: AtomicBool,
    flushing: AtomicBool,
}

impl Cancellation {
    pub(crate) fn new(parent: Option<Arc<Self>>, core: Weak<Core>) -> Arc<Self> {
        Arc::new(Self {
            requested: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
            parent,
            core,
            pending: AtomicBool::new(false),
            flushing: AtomicBool::new(false),
        })
    }

    pub(crate) fn request(&self) {
        if !self.requested.swap(true, Ordering::AcqRel) {
            self.pending.store(true, Ordering::Release);
        }
    }
    pub(crate) fn pending(&self) -> bool {
        self.pending.load(Ordering::Acquire) || self.flushing.load(Ordering::Acquire)
    }
    pub(crate) fn take_notification(&self) -> bool {
        if self.pending.swap(false, Ordering::AcqRel) {
            self.flushing.store(true, Ordering::Release);
            true
        } else {
            false
        }
    }
    pub(crate) fn flush(&self) -> Result<(), sugarsubstitute_contracts::execution::PanicCause> {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.notify.notify_waiters()));
        self.flushing.store(false, Ordering::Release);
        result.map_err(crate::task::panic_cause)
    }
}

impl CancelControl for Cancellation {
    fn is_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
            || self.parent.as_ref().is_some_and(|parent| parent.is_requested())
    }

    fn cancelled(&self) -> WaitFuture {
        let notify = Arc::clone(&self.notify);
        let requested = Arc::clone(&self.requested);
        let parent = self.parent.clone();
        let future: WaitFuture = Box::pin(async move {
            let wait = notify.notified_owned();
            tokio::pin!(wait);
            wait.as_mut().enable();
            if requested.load(Ordering::Acquire) {
                return;
            }
            match parent {
                Some(parent) => tokio::select! { () = wait => {}, () = parent.cancelled() => {} },
                None => wait.await,
            }
        });
        Box::pin(SupervisedWait { future, core: self.core.clone() })
    }
}

#[path = "scope_notification.rs"]
mod notification;
use notification::SupervisedWait;

pub(crate) struct ScopeState {
    pub(crate) spec: ScopeSpec,
    pub(crate) parent: Option<ScopeId>,
    pub(crate) deadline: Option<Moment>,
    pub(crate) cancel: Arc<Cancellation>,
    pub(crate) closed: bool,
    pub(crate) abandoned: bool,
    pub(crate) bytes: u64,
}

pub(crate) struct ScopeHandle {
    pub(crate) owner_id: ExecutionId,
    pub(crate) id: ScopeId,
    pub(crate) core: Weak<Core>,
}

impl ScopeControl for ScopeHandle {
    fn owner_id(&self) -> ExecutionId {
        self.owner_id
    }
    fn cancel(&self) {
        if let Some(core) = self.core.upgrade() {
            core.cancel_scope(self.id);
        }
    }
    fn snapshot(&self) -> ScopeSnapshot {
        self.core.upgrade().map_or(
            ScopeSnapshot {
                id: self.id,
                closed: true,
                cancellation_requested: true,
                queued: 0,
                running: 0,
                terminal_unobserved: 0,
                children_live: 0,
                reserved_bytes: 0,
            },
            |core| core.scope_snapshot(self.id),
        )
    }
    fn abandon(&self) {
        if let Some(core) = self.core.upgrade() {
            core.cancel_scope(self.id);
            core.close_scope(self.id);
        }
    }
}

pub(crate) fn earliest(left: Option<Moment>, right: Option<Moment>) -> Option<Moment> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (left, right) => left.or(right),
    }
}
