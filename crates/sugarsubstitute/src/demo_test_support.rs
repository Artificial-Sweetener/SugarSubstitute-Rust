// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Synchronize qualification tests through real execution completion notifications.

use super::*;
use std::{
    sync::{Condvar, Mutex},
    time::Instant,
};
use sugarsubstitute_contracts::execution::{ExecutionError, Moment, Operation, WaitFuture};

/// Observe execution-owned terminal wakes without creating another scheduler.
pub(super) struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}

impl Wake {
    /// Establish one test-owned completion observer.
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self { revision: Mutex::new(0), changed: Condvar::new() })
    }

    /// Capture the wake revision before attempting terminal transfer.
    fn revision(&self) -> Result<u64, Box<dyn Error>> {
        Ok(*self.revision.lock().map_err(|_| "Completion observer poisoned")?)
    }

    /// Wait for an actual notification with a failure deadline, preserving early wakes.
    fn wait_after(&self, revision: u64, deadline: Instant) -> Result<bool, Box<dyn Error>> {
        let mut current = self.revision.lock().map_err(|_| "Completion observer poisoned")?;
        while *current <= revision {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Ok(false);
            };
            let (next, timed) = self
                .changed
                .wait_timeout(current, remaining)
                .map_err(|_| "Completion observer poisoned")?;
            current = next;
            if timed.timed_out() && *current <= revision {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.revision.lock().map_err(|_| {
            ExecutionError::new(
                Operation::NativeLifecycle,
                Box::new(std::io::Error::other("Completion observer poisoned")),
            )
        })?;
        *revision = revision.checked_add(1).ok_or_else(|| {
            ExecutionError::new(
                Operation::NativeLifecycle,
                Box::new(std::io::Error::other("Completion observer exhausted")),
            )
        })?;
        self.changed.notify_all();
        Ok(())
    }
}

/// Drive a real admitted raster task and observe its typed immutable completion.
pub(super) fn prepare(
    demo: &mut SplashDemo,
    wake: &Wake,
    elapsed: Duration,
) -> Result<PreparedFrame, Box<dyn Error>> {
    demo.request_frame(FrameShape::new(558, 558, 1.0)?, elapsed)?;
    let deadline = Instant::now().checked_add(Duration::from_secs(10)).ok_or("Invalid deadline")?;
    loop {
        let revision = wake.revision()?;
        if let Some(frame) = demo.poll_frame(NonZeroUsize::MIN)? {
            return Ok(frame);
        }
        if !wake.wait_after(revision, deadline)? {
            return demo
                .poll_frame(NonZeroUsize::MIN)?
                .ok_or_else(|| "Preparation frame deadline expired".into());
        }
    }
}

/// Start only the real execution boundary; these tests own no native window or event loop.
pub(super) fn start(demo: &mut SplashDemo) -> Result<Arc<Wake>, Box<dyn Error>> {
    let wake = Wake::new();
    demo.clock = Arc::new(FrozenClock);
    demo.start(wake.clone())?;
    demo.set_visible(true)?;
    Ok(wake)
}

/// Supply a fixed execution epoch; elapsed presentation samples remain explicit test inputs.
struct FrozenClock;
impl Clock for FrozenClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

/// Observe the demo's actual two-phase shutdown without assuming a wake means workers stopped.
pub(super) fn finish(demo: &mut SplashDemo, wake: &Wake) -> Result<(), Box<dyn Error>> {
    demo.request_shutdown()?;
    let deadline = Instant::now().checked_add(Duration::from_secs(10)).ok_or("Invalid deadline")?;
    loop {
        let revision = wake.revision()?;
        if demo.poll_shutdown()? {
            return demo.finish_shutdown();
        }
        if !wake.wait_after(revision, deadline)? {
            if demo.poll_shutdown()? {
                return demo.finish_shutdown();
            }
            return Err("Demo execution settlement deadline expired".into());
        }
    }
}
