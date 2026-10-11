// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow a native proxy for completion wakeups only while its event loop is alive.

use super::NativeAction;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_contracts::execution::{CompletionWake, ExecutionError, Operation};
use tao::event_loop::EventLoopProxy;

/// Disconnect before native teardown, after the owner proves all worker callbacks settled.
pub(super) struct ProxyWake {
    proxy: EventLoopProxy<NativeAction>,
    lifetime: WakeLifetime,
}

/// Share only whether the native completion mailbox remains connected.
#[derive(Clone)]
pub(super) struct WakeLifetime(Arc<AtomicBool>);

impl WakeLifetime {
    /// Create a connected wake lifetime before preparation can emit early completions.
    pub(super) fn new() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }

    /// Reject subsequent wakes before controls/window/event-loop resources are dropped.
    pub(super) fn disconnect(&self) {
        self.0.store(false, Ordering::Release);
    }

    /// Preserve a typed native lifecycle failure when an owner uses a closed mailbox.
    fn check(&self) -> Result<(), ExecutionError> {
        if self.0.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(ExecutionError::new(
                Operation::NativeLifecycle,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "Native completion wake is disconnected",
                )),
            ))
        }
    }
}

impl ProxyWake {
    /// Retain Tao's thread-safe proxy without lending a native window to workers.
    pub(super) fn new(proxy: EventLoopProxy<NativeAction>, lifetime: WakeLifetime) -> Self {
        Self { proxy, lifetime }
    }
}

impl CompletionWake for ProxyWake {
    fn wake(&self) -> Result<(), ExecutionError> {
        self.lifetime.check()?;
        self.proxy
            .send_event(NativeAction::Completions)
            .map_err(|cause| ExecutionError::new(Operation::NativeLifecycle, Box::new(cause)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    /// Closing is shared and irreversible, and failures retain their native operation/cause.
    #[test]
    fn disconnected_completion_wake_preserves_native_lifecycle_cause() -> Result<(), Box<dyn Error>>
    {
        let lifetime = WakeLifetime::new();
        let retained = lifetime.clone();
        lifetime.check()?;
        lifetime.disconnect();
        retained.disconnect();
        let error = retained.check().err().ok_or("Closed wake was accepted")?;
        assert_eq!(error.operation(), Operation::NativeLifecycle);
        let cause = error
            .source()
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .ok_or("Original closed mailbox error was lost")?;
        assert_eq!(cause.kind(), std::io::ErrorKind::BrokenPipe);
        Ok(())
    }
}
