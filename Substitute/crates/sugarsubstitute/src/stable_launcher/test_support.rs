// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold a launcher job inside its body until the test releases it, ignoring cancellation, as a
//! native call that never returns would.
use std::sync::{Arc, Condvar, Mutex, PoisonError};

#[derive(Default)]
struct State {
    entered: bool,
    released: bool,
}

/// One job body's explicit hold.
#[derive(Default)]
pub(super) struct Hold {
    state: Mutex<State>,
    changed: Condvar,
}

impl Hold {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
    /// Run inside the job body: report entry, then block until released.
    pub(super) fn block(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.entered = true;
        self.changed.notify_all();
        while !state.released {
            state = self.changed.wait(state).unwrap_or_else(PoisonError::into_inner);
        }
    }
    /// Wait until the worker runs the body, so cancellation cannot retire the job unstarted.
    pub(super) fn entered(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        while !state.entered {
            state = self.changed.wait(state).unwrap_or_else(PoisonError::into_inner);
        }
    }
    /// Let the held body return.
    pub(super) fn release(&self) {
        self.state.lock().unwrap_or_else(PoisonError::into_inner).released = true;
        self.changed.notify_all();
    }
}

/// Releases its hold when dropped, so a failing assertion cannot leave the worker blocked and
/// the executor's teardown waiting for it.
pub(super) struct Released(pub(super) Arc<Hold>);
impl Drop for Released {
    fn drop(&mut self) {
        self.0.release();
    }
}
