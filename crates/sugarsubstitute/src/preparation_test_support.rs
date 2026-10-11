// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Observe real preparation outcomes and cancellation with test-owned notifications.

use super::*;
use std::{
    sync::{Condvar, Mutex},
    time::Instant,
};
use sugarsubstitute_contracts::execution::{ExecutionError, JobCompletion, JobPhase};

/// Bound failures without assigning elapsed sleeps to asynchronous progress.
fn deadline() -> Result<Instant, Box<dyn Error>> {
    Instant::now()
        .checked_add(Duration::from_secs(10))
        .ok_or_else(|| "Invalid test deadline".into())
}

/// Keep terminal notifications observable even when they arrive before the next poll.
pub(super) struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl Wake {
    fn new() -> Arc<Self> {
        Arc::new(Self { revision: Mutex::new(0), changed: Condvar::new() })
    }
    fn revision(&self) -> Result<u64, Box<dyn Error>> {
        Ok(*self.revision.lock().map_err(|_| "Preparation observer poisoned")?)
    }
    /// Return false only at the failure deadline; callers perform one final real observation.
    fn wait_after(&self, revision: u64, deadline: Instant) -> Result<bool, Box<dyn Error>> {
        let mut current = self.revision.lock().map_err(|_| "Preparation observer poisoned")?;
        while *current <= revision {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Ok(false);
            };
            let (next, timed) = self
                .changed
                .wait_timeout(current, remaining)
                .map_err(|_| "Preparation observer poisoned")?;
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
                Box::new(std::io::Error::other("Preparation observer poisoned")),
            )
        })?;
        *revision = revision.checked_add(1).ok_or_else(|| {
            ExecutionError::new(
                Operation::NativeLifecycle,
                Box::new(std::io::Error::other("Preparation observer exhausted")),
            )
        })?;
        self.changed.notify_all();
        Ok(())
    }
}

/// Start a real isolated execution lifetime without a native window.
pub(super) fn started() -> Result<(PreparationDriver, Arc<Wake>), Box<dyn Error>> {
    let mut driver = PreparationDriver::new("en")?;
    let wake = Wake::new();
    driver.start(wake.clone())?;
    driver.set_visible(true)?;
    Ok((driver, wake))
}

/// Submit one plain-data render request through production admission.
pub(super) fn request(
    driver: &mut PreparationDriver,
    completed: f32,
) -> Result<(), Box<dyn Error>> {
    driver.request(
        FrameShape::new(558, 558, 1.0)?,
        Appearance::demo(),
        PaperFlip::new(1).frame(Duration::ZERO),
        ProgressFrame { completed, activity: None },
    )
}

/// Transfer an admitted current frame using actual typed terminal publication.
pub(super) fn prepare(
    driver: &mut PreparationDriver,
    wake: &Wake,
    completed: f32,
) -> Result<PreparedFrame, Box<dyn Error>> {
    request(driver, completed)?;
    pending_frame(driver, wake)
}

/// Wait for the retained request without sampling a replacement request.
pub(super) fn pending_frame(
    driver: &mut PreparationDriver,
    wake: &Wake,
) -> Result<PreparedFrame, Box<dyn Error>> {
    let deadline = deadline()?;
    loop {
        let revision = wake.revision()?;
        if let Some(frame) = driver.poll(NonZeroUsize::MIN)? {
            return Ok(frame);
        }
        if !wake.wait_after(revision, deadline)? {
            return driver
                .poll(NonZeroUsize::MIN)?
                .ok_or_else(|| "Preparation frame deadline expired".into());
        }
    }
}

/// Observe terminal status without taking the driver's actual completion prematurely.
pub(super) fn terminal(driver: &PreparationDriver, wake: &Wake) -> Result<(), Box<dyn Error>> {
    let deadline = deadline()?;
    loop {
        let revision = wake.revision()?;
        let job = &driver.active.as_ref().ok_or("Missing active job")?.job;
        if matches!(job.status().phase, JobPhase::Terminal(_)) {
            return Ok(());
        }
        if !wake.wait_after(revision, deadline)? {
            return if matches!(job.status().phase, JobPhase::Terminal(_)) {
                Ok(())
            } else {
                Err("Preparation terminal deadline expired".into())
            };
        }
    }
}

/// Acknowledge an external production-port fixture's terminal result and retained charge.
pub(super) fn completion<T, E>(
    job: &mut OwnedJob<T, E>,
    wake: &Wake,
) -> Result<JobCompletion<T, E>, Box<dyn Error>> {
    let deadline = deadline()?;
    loop {
        let revision = wake.revision()?;
        if let Some(completion) = job.try_take_completion() {
            return Ok(completion);
        }
        if !wake.wait_after(revision, deadline)? {
            return job
                .try_take_completion()
                .ok_or_else(|| "Fixture completion deadline expired".into());
        }
    }
}

/// Prove stopped workers while retained frame/cache memory remains independently owned.
pub(super) fn settle(driver: &mut PreparationDriver, wake: &Wake) -> Result<(), Box<dyn Error>> {
    driver.request_shutdown()?;
    let deadline = deadline()?;
    loop {
        let revision = wake.revision()?;
        if driver.poll_shutdown()? {
            return Ok(());
        }
        if !wake.wait_after(revision, deadline)? {
            // Worker exit can follow its final terminal notification. Inspect real
            // settlement once at the failure boundary rather than treating a wake as stop.
            return if driver.poll_shutdown()? {
                Ok(())
            } else {
                Err("Preparation shutdown deadline expired".into())
            };
        }
    }
}

/// Hold actual finite blocking work until the test explicitly releases its cancellation boundary.
pub(super) struct BlockingLatch {
    state: Mutex<(bool, bool)>,
    changed: Condvar,
}
impl BlockingLatch {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self { state: Mutex::new((false, false)), changed: Condvar::new() })
    }
    pub(super) fn run(&self) -> Result<(), std::io::Error> {
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(10))
            .ok_or_else(|| std::io::Error::other("Invalid fixture deadline"))?;
        let mut state =
            self.state.lock().map_err(|_| std::io::Error::other("Blocking fixture poisoned"))?;
        state.0 = true;
        self.changed.notify_all();
        while !state.1 {
            let remaining = deadline.checked_duration_since(Instant::now()).ok_or_else(|| {
                std::io::Error::other("Blocking fixture release deadline expired")
            })?;
            let (next, timed) = self
                .changed
                .wait_timeout(state, remaining)
                .map_err(|_| std::io::Error::other("Blocking fixture poisoned"))?;
            state = next;
            if timed.timed_out() && !state.1 {
                return Err(std::io::Error::other("Blocking fixture release deadline expired"));
            }
        }
        Ok(())
    }
    pub(super) fn wait_started(&self) -> Result<(), Box<dyn Error>> {
        let deadline = deadline()?;
        let mut state = self.state.lock().map_err(|_| "Blocking fixture poisoned")?;
        while !state.0 {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("Blocking fixture start deadline expired")?;
            let (next, timed) = self
                .changed
                .wait_timeout(state, remaining)
                .map_err(|_| "Blocking fixture poisoned")?;
            state = next;
            if timed.timed_out() && !state.0 {
                return Err("Blocking fixture start deadline expired".into());
            }
        }
        Ok(())
    }
    pub(super) fn release(&self) -> Result<(), Box<dyn Error>> {
        self.state.lock().map_err(|_| "Blocking fixture poisoned")?.1 = true;
        self.changed.notify_all();
        Ok(())
    }
}
