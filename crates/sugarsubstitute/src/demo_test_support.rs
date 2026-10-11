// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Synchronize qualification tests through real execution completion notifications.

use super::*;
use std::{
    sync::{Condvar, Mutex},
    time::Instant,
};
use sugarsubstitute_contracts::execution::{ExecutionError, Operation};

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
    fn wait_after(&self, revision: u64, deadline: Instant) -> Result<(), Box<dyn Error>> {
        let mut current = self.revision.lock().map_err(|_| "Completion observer poisoned")?;
        while *current <= revision {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("Preparation did not publish within the qualification deadline")?;
            let (next, timed) = self
                .changed
                .wait_timeout(current, remaining)
                .map_err(|_| "Completion observer poisoned")?;
            current = next;
            if timed.timed_out() && *current <= revision {
                return Err("Preparation did not signal within the qualification deadline".into());
            }
        }
        Ok(())
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
        wake.wait_after(revision, deadline)?;
    }
}

/// Start only the real execution boundary; these tests own no native window or event loop.
pub(super) fn start(demo: &mut SplashDemo) -> Result<Arc<Wake>, Box<dyn Error>> {
    let wake = Wake::new();
    demo.start(wake.clone())?;
    demo.set_visible(true)?;
    Ok(wake)
}
