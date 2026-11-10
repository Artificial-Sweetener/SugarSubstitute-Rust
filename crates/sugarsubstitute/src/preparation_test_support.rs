// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Observe real preparation outcomes and cancellation with test-owned notifications.

use super::*;
use crate::completion_wake::{Gate, Wake};
use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionNotification, CompletionWake, ExecutionError, JobId, Moment, ScopeId,
    ScopeSpec, WaitFuture,
};
use sugarsubstitute_contracts::execution::{JobCompletion, JobPhase};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};

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
pub(super) struct CpuHold(pub(super) Arc<Gate>);
impl sugarsubstitute_contracts::execution::CpuTask<(), std::io::Error> for CpuHold {
    fn step(
        &mut self,
        _: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<sugarsubstitute_contracts::execution::CpuStep<()>, std::io::Error> {
        self.0.hold();
        Ok(sugarsubstitute_contracts::execution::CpuStep::Complete(()))
    }
}

/// Keep execution time deterministic; progress is observed only through completion wakes.
struct FrozenClock;
impl Clock for FrozenClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

/// Observe the execution owner as its host does, holding drained notices for the driver.
///
/// The owner coalesces its wakes until its mailbox is drained, so a test that waits for one job
/// while another job's notice sits undrained would never be woken. Every wait here therefore
/// drains the mailbox first, which re-arms the wake, and keeps what it drained in order. A notice
/// reaches the driver only when the test routes it with `poll`, `pump` or `route_all`, so tests
/// that assert what the driver holds before it sees a terminal notice stay in control.
pub(super) struct Host {
    wake: Wake,
    custody: Mutex<VecDeque<CompletionNotification>>,
}

impl CompletionWake for Host {
    fn wake(&self) -> Result<(), ExecutionError> {
        self.wake.wake()
    }
}

impl Host {
    fn custody(&self) -> MutexGuard<'_, VecDeque<CompletionNotification>> {
        self.custody.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Move every notice the mailbox holds into custody, which re-arms its next wake.
    fn collect(&self, client: &ExecutionClient) -> Result<(), ExecutionError> {
        loop {
            let batch = client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            self.custody().extend(batch.notifications);
            if batch.remaining == 0 {
                return Ok(());
            }
        }
    }

    /// Drop a job's notice once its completion was transferred, as the owner does for its mailbox.
    fn forget(&self, job: JobId) {
        self.custody().retain(|notice| notice.job_id != job);
    }

    /// Take the oldest notice the driver has not seen: from custody first, then the mailbox.
    fn next(&self, client: &ExecutionClient) -> Result<CompletionBatch, ExecutionError> {
        let held = self.custody().pop_front();
        match held {
            Some(notice) => {
                Ok(CompletionBatch { notifications: vec![notice], remaining: self.custody().len() })
            }
            None => client.drain_completions(NonZeroUsize::MIN),
        }
    }
}

/// Construct the real execution boundary with a deterministic injected epoch.
pub(super) fn started() -> Result<(PreparationDriver, Arc<Host>, ExecutionOwner), Box<dyn Error>> {
    let wake = Arc::new(Host { wake: Wake::default(), custody: Mutex::new(VecDeque::new()) });
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

/// Route the oldest undelivered notice through the rendering consumer.
pub(super) fn poll(
    driver: &mut PreparationDriver,
    host: &Host,
) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
    let batch = host.next(&driver.client)?;
    driver.consume(&batch)
}

/// Route notices one at a time until one publishes a frame or none remain.
fn pump(
    driver: &mut PreparationDriver,
    host: &Host,
) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
    loop {
        let batch = host.next(&driver.client)?;
        if let Some(frame) = driver.consume(&batch)? {
            return Ok(Some(frame));
        }
        if batch.notifications.is_empty() {
            return Ok(None);
        }
    }
}

/// Route every undelivered notice, discarding any frame they publish.
fn route_all(driver: &mut PreparationDriver, host: &Host) -> Result<(), Box<dyn Error>> {
    loop {
        let batch = host.next(&driver.client)?;
        driver.consume(&batch)?;
        if batch.notifications.is_empty() {
            return Ok(());
        }
    }
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
    host: &Host,
    completed: f32,
) -> Result<PreparedFrame, Box<dyn Error>> {
    request(driver, completed)?;
    pending_frame(driver, host)
}

/// Wait for the retained request without sampling a replacement request.
pub(super) fn pending_frame(
    driver: &mut PreparationDriver,
    host: &Host,
) -> Result<PreparedFrame, Box<dyn Error>> {
    host.wake.until(|| pump(driver, host).transpose())
}

/// Observe terminal status without taking the driver's actual completion prematurely.
pub(super) fn terminal(driver: &PreparationDriver, host: &Host) -> Result<(), Box<dyn Error>> {
    let job = &driver.active.as_ref().ok_or("Missing active job")?.job;
    host.wake.until(|| match host.collect(&driver.client) {
        Err(cause) => Some(Err(cause.into())),
        Ok(()) => matches!(job.status().phase, JobPhase::Terminal(_)).then_some(Ok(())),
    })
}

/// Acknowledge an external production-port fixture's terminal result and retained charge.
pub(super) fn completion<T, E>(
    client: &ExecutionClient,
    job: &mut OwnedJob<T, E>,
    host: &Host,
) -> Result<JobCompletion<T, E>, Box<dyn Error>> {
    host.wake.until(|| match host.collect(client) {
        Err(cause) => Some(Err(cause.into())),
        Ok(()) => {
            let completion = job.try_take_completion();
            if completion.is_some() {
                host.forget(job.spec().id);
            }
            completion.map(Ok)
        }
    })
}

/// Prove rendering work stopped while retained frame/cache memory remains independently owned.
pub(super) fn settle(driver: &mut PreparationDriver, host: &Host) -> Result<(), Box<dyn Error>> {
    driver.request_retirement()?;
    // Worker exit can follow its final terminal notification, so a wake is never taken as stop:
    // each wake only prompts another real observation of retirement.
    host.wake.until(|| {
        let retired = route_all(driver, host).and_then(|()| driver.poll_retirement());
        retired.map(|retired| retired.then_some(())).transpose()
    })
}

/// Retire the sole execution owner after rendering; retain real joins until observed settlement.
pub(super) fn finish(
    owner: &mut ExecutionOwner,
    driver: &mut PreparationDriver,
    host: &Host,
) -> Result<(), Box<dyn Error>> {
    settle(driver, host)?;
    driver.finish_retirement()?;
    owner.request_shutdown(Moment::from_nanos(5_000_000_000));
    host.wake.until(|| {
        let settled = route_all(driver, host).and_then(|()| {
            owner.poll_shutdown().map(|progress| progress.settled).map_err(Into::into)
        });
        settled.map(|settled| settled.then_some(())).transpose()
    })?;
    Ok(owner.finish_shutdown()?)
}
