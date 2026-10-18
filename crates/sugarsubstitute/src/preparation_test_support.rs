// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Observe real preparation outcomes and cancellation with test-owned notifications.

use super::*;
use std::{num::NonZeroUsize, sync::Arc, time::Duration};
use std::{
    sync::{Condvar, Mutex},
    time::Instant,
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, Moment, ScopeId, ScopeSpec, WaitFuture,
};
use sugarsubstitute_contracts::execution::{ExecutionError, JobCompletion, JobPhase};
use sugarsubstitute_execution::ExecutionOwner;

/// Preserve real snapshot admission while tests vary only inert resource generation.
pub(super) fn transcript(
    generation: u64,
) -> Result<sugarsubstitute_application::transcript::TranscriptStore, Box<dyn Error>> {
    use sugarsubstitute_contracts::lifecycle::*;
    Ok(sugarsubstitute_application::transcript::TranscriptStore::new(ResourceKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(generation)?,
    }))
}

/// Hold real global memory admission without blocking a worker or sleeping.
pub(super) fn saturate(
    driver: &PreparationDriver,
    owner: &ExecutionOwner,
) -> Result<OwnedJob<(), Infallible>, Box<dyn Error>> {
    let client = &driver.client;
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    Ok(client.submit_blocking(
        &scope,
        JobSpec {
            id: driver.jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: None,
            reservation_bytes: owner.limits().memory_bytes,
            replacement: None,
        },
        |_| Ok::<(), Infallible>(()),
    )?)
}

/// Occupy a real CPU worker at a controlled finite external boundary.
pub(super) struct CpuHold(pub(super) Arc<BlockingLatch>);
impl sugarsubstitute_contracts::execution::CpuTask<(), std::io::Error> for CpuHold {
    fn step(
        &mut self,
        _: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<sugarsubstitute_contracts::execution::CpuStep<()>, std::io::Error> {
        self.0.run()?;
        Ok(sugarsubstitute_contracts::execution::CpuStep::Complete(()))
    }
}

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

/// Keep execution time deterministic; synchronization deadlines only bound test failures.
struct FrozenClock;
impl Clock for FrozenClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

/// Construct the real execution boundary with a deterministic injected epoch.
pub(super) fn started() -> Result<(PreparationDriver, Arc<Wake>, ExecutionOwner), Box<dyn Error>> {
    let wake = Wake::new();
    let owner = ExecutionOwner::new(Arc::new(FrozenClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut driver = PreparationDriver::new(
        "en",
        client,
        scope,
        JobSequence::new(),
        sugarsubstitute_contracts::presentation::ConsolePresentation::Raster,
        crate::preparation::FirstFrameContent::Character,
    )?;
    driver.set_visible(true)?;
    Ok((driver, wake, owner))
}

/// Route one centrally drained batch through the rendering consumer.
pub(super) fn poll(
    driver: &mut PreparationDriver,
) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
    let batch = driver.client.drain_completions(NonZeroUsize::MIN)?;
    driver.consume(&batch)
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
        None,
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
        if let Some(frame) = poll(driver)? {
            return Ok(frame);
        }
        if !wake.wait_after(revision, deadline)? {
            return poll(driver)?.ok_or_else(|| "Preparation frame deadline expired".into());
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

/// Prove rendering work stopped while retained frame/cache memory remains independently owned.
pub(super) fn settle(driver: &mut PreparationDriver, wake: &Wake) -> Result<(), Box<dyn Error>> {
    driver.request_retirement()?;
    let deadline = deadline()?;
    loop {
        let revision = wake.revision()?;
        poll(driver)?;
        if driver.poll_retirement()? {
            return Ok(());
        }
        if !wake.wait_after(revision, deadline)? {
            // Worker exit can follow its final terminal notification. Inspect real
            // settlement once at the failure boundary rather than treating a wake as stop.
            poll(driver)?;
            return if driver.poll_retirement()? {
                Ok(())
            } else {
                Err("Preparation retirement deadline expired".into())
            };
        }
    }
}

/// Retire the sole execution owner after rendering; retain real joins until observed settlement.
pub(super) fn finish(
    owner: &mut ExecutionOwner,
    driver: &mut PreparationDriver,
    wake: &Wake,
) -> Result<(), Box<dyn Error>> {
    settle(driver, wake)?;
    driver.finish_retirement()?;
    owner.request_shutdown(Moment::from_nanos(5_000_000_000));
    let deadline = deadline()?;
    loop {
        let revision = wake.revision()?;
        let batch = owner.client().drain_completions(NonZeroUsize::MIN)?;
        driver.consume(&batch)?;
        if owner.poll_shutdown()?.settled {
            return Ok(owner.finish_shutdown()?);
        }
        if !wake.wait_after(revision, deadline)? {
            if owner.poll_shutdown()?.settled {
                return Ok(owner.finish_shutdown()?);
            }
            return Err("Execution settlement deadline expired".into());
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
