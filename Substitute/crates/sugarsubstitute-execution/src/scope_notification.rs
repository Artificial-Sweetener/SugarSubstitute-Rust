// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Supervise each supplied cancellation waker without interrupting sibling wake delivery.

use super::*;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Wake, Waker},
};
use sugarsubstitute_contracts::execution::{ExecutionError, Operation};

struct SupervisedWake {
    target: Option<Waker>,
    core: Weak<Core>,
}
impl Wake for SupervisedWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Some(target) = &self.target {
                target.wake_by_ref();
            }
        })) {
            let cause = crate::task::panic_cause(payload);
            if let Some(core) = self.core.upgrade() {
                core.mailbox.record_failure(ExecutionError::new(
                    Operation::Infrastructure,
                    Box::new(std::io::Error::other(cause.as_str().to_owned())),
                ));
                core.mailbox.deliver_wake();
                core.changed();
            }
        }
    }
}

impl Drop for SupervisedWake {
    fn drop(&mut self) {
        let target = self.target.take();
        if let Err(payload) =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(target)))
        {
            let cause = crate::task::panic_cause(payload);
            if let Some(core) = self.core.upgrade() {
                core.mailbox.record_failure(ExecutionError::new(
                    Operation::Infrastructure,
                    Box::new(std::io::Error::other(cause.as_str().to_owned())),
                ));
                core.mailbox.deliver_wake();
                core.changed();
            }
        }
    }
}

pub(super) struct SupervisedWait {
    pub(super) future: WaitFuture,
    pub(super) core: Weak<Core>,
}
impl Future for SupervisedWait {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        let waker = Waker::from(Arc::new(SupervisedWake {
            target: Some(context.waker().clone()),
            core: self.core.clone(),
        }));
        self.future.as_mut().poll(&mut Context::from_waker(&waker))
    }
}
