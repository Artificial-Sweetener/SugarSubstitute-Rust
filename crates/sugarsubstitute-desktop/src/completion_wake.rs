// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow a native proxy for completion wakeups only while its event loop is alive.

#[cfg(all(windows, test))]
pub(crate) mod observer;

use std::fmt::Debug;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_contracts::execution::{CompletionWake, ExecutionError, Operation};
use tao::event_loop::EventLoopProxy;

/// Disconnect before native teardown, after the owner proves all worker callbacks settled.
pub(crate) struct ProxyWake<T: 'static> {
    proxy: EventLoopProxy<T>,
    lifetime: WakeLifetime,
    completion: T,
}

/// Share only whether the native completion mailbox remains connected.
#[derive(Clone)]
pub(crate) struct WakeLifetime(Arc<AtomicBool>);

impl WakeLifetime {
    /// Create a connected wake lifetime before preparation can emit early completions.
    pub(crate) fn new() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }

    /// Reject subsequent wakes before controls/window/event-loop resources are dropped.
    pub(crate) fn disconnect(&self) {
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

impl<T: 'static> ProxyWake<T> {
    /// Retain Tao's thread-safe proxy without lending a native window to workers.
    pub(crate) fn new(proxy: EventLoopProxy<T>, lifetime: WakeLifetime, completion: T) -> Self {
        Self { proxy, lifetime, completion }
    }
}

impl<T: Clone + Debug + Send + Sync + 'static> CompletionWake for ProxyWake<T> {
    fn wake(&self) -> Result<(), ExecutionError> {
        self.lifetime.check()?;
        self.proxy
            .send_event(self.completion.clone())
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
