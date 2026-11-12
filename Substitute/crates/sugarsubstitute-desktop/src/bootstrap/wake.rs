// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serialize primitive signalling, target publication and disconnect through one endpoint.
use super::EventBoundary;
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::execution::{CompletionWake, ExecutionError, Operation};

enum Target {
    Event(Arc<dyn EventBoundary>),
    Proxy(Arc<dyn CompletionWake>),
    Closed,
}
struct State {
    target: Target,
    pending: bool,
}
/// The main-thread adapter retains the event separately through checked native closure.
pub(crate) struct BootstrapWake(Mutex<State>);
fn error(message: &'static str) -> ExecutionError {
    ExecutionError::new(Operation::NativeLifecycle, Box::new(std::io::Error::other(message)))
}
impl BootstrapWake {
    pub(super) fn new(event: Arc<dyn EventBoundary>) -> Self {
        Self(Mutex::new(State { target: Target::Event(event), pending: false }))
    }
    pub(super) fn begin_turn(&self) -> Result<(), ExecutionError> {
        let mut state = self.0.lock().map_err(|_| error("Bootstrap wake lock poisoned"))?;
        if state.pending
            && let Target::Event(event) = &state.target
        {
            event.reset()?;
        }
        state.pending = false;
        Ok(())
    }
    pub(super) fn retarget(&self, target: Arc<dyn CompletionWake>) -> Result<(), ExecutionError> {
        let mut state = self.0.lock().map_err(|_| error("Bootstrap wake lock poisoned"))?;
        if !matches!(state.target, Target::Event(_)) {
            return Err(error("Bootstrap wake target already handed off or closed"));
        }
        // Queue one actual completion turn even when bounded preparation left notices.
        // The lock keeps worker signals away from the event until publication commits.
        target.wake()?;
        state.target = Target::Proxy(target);
        state.pending = false;
        Ok(())
    }
    pub(super) fn disconnect(&self) -> Result<(), ExecutionError> {
        let mut state = self.0.lock().map_err(|_| error("Bootstrap wake lock poisoned"))?;
        state.target = Target::Closed;
        Ok(())
    }
}
impl CompletionWake for BootstrapWake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut state = self.0.lock().map_err(|_| error("Bootstrap wake lock poisoned"))?;
        state.pending = true;
        match &state.target {
            Target::Event(event) => event.signal(),
            Target::Proxy(proxy) => proxy.wake(),
            Target::Closed => Err(error("Bootstrap completion wake is disconnected")),
        }
    }
}
