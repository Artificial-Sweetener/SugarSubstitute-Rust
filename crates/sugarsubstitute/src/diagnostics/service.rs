// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose finite persistent batches through the established execution owner for every process role.
//!
//! Execution limits belong to each `ExecutionOwner`; the adapter has no process-global byte ledger.
//! The supported process entry creates this private instance once, before role startup, and joins
//! it after role teardown. Its only submissions are one one-MiB pump and one eight-MiB publication
//! at a time. Their nine-MiB peak is additional to the role's own ledger. Original per-owner limits
//! stay unchanged. The instance also owns the adapter's fixed async, blocking, CPU and native worker
//! facilities; none of those handles or the private submission client escapes this module.
use super::{
    DiagnosticHealth,
    queue::{EVENT_BYTES, QUEUE_EVENTS, Queue},
};
use std::{
    io,
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionError, ExecutionPort, JobClass, JobContext, JobId, JobOutcome,
    JobSpec, Operation, OwnedJob, Scope, ScopeId, ScopeSpec,
};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock};

/// Original reservation retaining the queue and pump control for the process lifetime.
const PUMP_MEMORY_BYTES: u64 = 1024 * 1024;
/// One finite native publication, including bounded protected storage preparation.
const BATCH_MEMORY_BYTES: u64 = 8 * 1024 * 1024;

/// Actual persistence boundary; producers never receive its filesystem or scheduling authority.
pub(super) trait Sink: Send + Sync + 'static {
    /// Publish one admitted bounded batch on its finite execution worker.
    fn append(&self, frames: &[String], omitted: u64, context: &JobContext) -> io::Result<()>;
    /// Classify this boundary's explicit writer admission failure, never arbitrary sink errors.
    fn overloaded(&self, _cause: &io::Error) -> bool {
        false
    }
}
/// Publish only through the established protected operational history port, below the
/// installation-local base that the process role selected.
pub(super) struct AccountSink {
    pub(super) base: std::path::PathBuf,
}
impl Sink for AccountSink {
    fn overloaded(&self, cause: &io::Error) -> bool {
        sugarsubstitute_supervision::diagnostic_storage::operational::writer_busy(cause)
    }
    fn append(&self, frames: &[String], omitted: u64, context: &JobContext) -> io::Result<()> {
        sugarsubstitute_supervision::diagnostic_storage::operational::append(
            &self.base, frames, omitted, context,
        )
    }
}

/// Wake the process-lifetime observer on actual execution completion activity.
#[derive(Default)]
struct Wake(Mutex<u64>, Condvar);
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *revision = revision.saturating_add(1);
        self.1.notify_all();
        Ok(())
    }
}
impl Wake {
    /// Wait only for actual completion activity or the original shutdown bound.
    fn wait(&self, until: Instant) -> io::Result<()> {
        let revision = self.0.lock().map_err(|_| io::Error::other("Diagnostic wake poisoned"))?;
        let value = *revision;
        let remaining =
            until.saturating_duration_since(Instant::now()).min(Duration::from_millis(10));
        let (_guard, _timeout) = self
            .1
            .wait_timeout_while(revision, remaining, |current| *current == value)
            .map_err(|_| io::Error::other("Diagnostic wake poisoned"))?;
        Ok(())
    }
}
/// Retain the execution scope, actual pump result and joins until after native application teardown.
pub(super) struct Service {
    owner: ExecutionOwner,
    client: ExecutionClient,
    queue: Arc<Queue>,
    job: OwnedJob<(), io::Error>,
    wake: Arc<Wake>,
    clock: Arc<MonotonicClock>,
}
impl Service {
    /// Admit the process route before producers start, using only the existing execution adapter.
    pub(super) fn start(
        health: DiagnosticHealth,
        sink: Arc<dyn Sink>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let queue = Queue::new();
        let clock = MonotonicClock::new();
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = owner.client();
        let scope = Arc::new(
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?,
        );
        let worker_scope = scope.clone();
        let worker_client = client.clone();
        let worker_queue = queue.clone();
        let startup_health = health.clone();
        let job = client.submit_async(
            &scope,
            spec(1, JobClass::IoOrdinary, None, PUMP_MEMORY_BYTES)?,
            move |context| async move {
                pump(worker_queue, worker_client, worker_scope, sink, health.clone(), context)
                    .await
                    .map_err(|cause| {
                        let kind = cause.kind();
                        if !cause.get_ref().is_some_and(|cause| cause.is::<ObservedSinkFailure>()) {
                            health.retain(cause);
                        }
                        io::Error::from(kind)
                    })
            },
        )?;
        let mut service = Self { owner, client, queue, job, wake, clock };
        let until = Instant::now() + Duration::from_secs(5);
        while service.queue.state().memory.is_none() {
            service.client.drain_completions(NonZeroUsize::MIN)?;
            if service.job.try_take_completion().is_some() {
                startup_health.check()?;
                return Err(io::Error::other("Diagnostic routing admission failed").into());
            }
            if Instant::now() >= until {
                service.job.cancel();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Diagnostic routing admission elapsed",
                )
                .into());
            }
            service.wake.wait(until)?;
        }
        Ok(service)
    }
    /// Give the supported formatter bounded producer access without scheduler or filesystem authority.
    pub(super) fn queue(&self) -> Arc<Queue> {
        self.queue.clone()
    }
    /// Flush admitted frames, observe the pump and finite storage outcomes, then join actual workers.
    pub(super) fn finish(mut self, health: &DiagnosticHealth) {
        match self.clock.now().checked_add(Duration::from_secs(4)) {
            Ok(cutoff) => self.queue.stop(cutoff),
            Err(cause) => {
                health.retain(io::Error::other(cause));
                self.queue.stop(self.clock.now());
            }
        }
        if let Err(cause) = self.finish_inner(health) {
            self.job.cancel();
            health.retain(cause);
        }
        self.queue.state().clear();
    }
    /// Observe terminal outcomes and actual native joins within one original shutdown observation bound.
    fn finish_inner(&mut self, health: &DiagnosticHealth) -> io::Result<()> {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            self.client
                .drain_completions(
                    NonZeroUsize::new(32)
                        .ok_or_else(|| io::Error::other("Invalid completion bound"))?,
                )
                .map_err(io::Error::other)?;
            if let Some(completion) = self.job.try_take_completion() {
                match completion.outcome {
                    JobOutcome::Success(()) => {}
                    JobOutcome::Failure(cause) => {
                        // The factory latched the original failure before returning its closed rejection.
                        if health.check().is_ok() {
                            health.retain(cause);
                        }
                    }
                    other => {
                        health.retain(io::Error::other(format!(
                            "Diagnostic pump terminal: {:?}",
                            other.kind()
                        )));
                    }
                }
                break;
            }
            if Instant::now() >= until {
                self.job.cancel();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Diagnostic flush deadline elapsed",
                ));
            }
            self.wake.wait(until)?;
        }
        self.owner.request_shutdown(
            self.clock.now().checked_add(Duration::from_secs(2)).map_err(io::Error::other)?,
        );
        while !self.owner.poll_shutdown().map_err(io::Error::other)?.settled {
            self.client.drain_completions(NonZeroUsize::MIN).map_err(io::Error::other)?;
            if Instant::now() >= until {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Diagnostic worker shutdown deadline elapsed",
                ));
            }
            self.wake.wait(until)?;
        }
        self.owner.finish_shutdown().map_err(io::Error::other)
    }
}
/// Build one typed finite reservation without exposing the scheduler to event producers.
fn spec(
    id: u64,
    class: JobClass,
    deadline: Option<sugarsubstitute_contracts::execution::Moment>,
    reservation_bytes: u64,
) -> io::Result<JobSpec> {
    Ok(JobSpec {
        id: JobId::new(id).map_err(io::Error::other)?,
        operation: Operation::Infrastructure,
        class,
        deadline,
        reservation_bytes,
        replacement: None,
    })
}
/// Release buffered allocations on every pump terminal, including cancellation.
struct PumpClaim(Arc<Queue>);
/// Closed rejection; the identical original source is already retained in diagnostic health.
#[derive(Debug)]
struct ObservedSinkFailure;
impl std::fmt::Display for ObservedSinkFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Owned diagnostic sink failed")
    }
}
impl std::error::Error for ObservedSinkFailure {}
impl Drop for PumpClaim {
    fn drop(&mut self) {
        self.0.state().clear();
    }
}
/// Drain redacted frames through finite admitted jobs while retaining original queue memory.
async fn pump(
    queue: Arc<Queue>,
    client: ExecutionClient,
    scope: Arc<Scope>,
    sink: Arc<dyn Sink>,
    health: DiagnosticHealth,
    context: JobContext,
) -> io::Result<()> {
    let _claim = PumpClaim(queue.clone());
    {
        let mut state = queue.state();
        state.memory = Some(Arc::new(
            context
                .memory()
                .claim((QUEUE_EVENTS * EVENT_BYTES + 65536) as u64)
                .map_err(io::Error::other)?,
        ));
        state.execution = Some(context.owner_id());
    }
    let mut sequence = 1_u64;
    loop {
        let batch = queue.take();
        let finished = batch.finished;
        if !batch.frames.is_empty() || batch.omitted != 0 {
            batches::publish(batch, &client, &scope, &sink, &health, &context, &mut sequence)
                .await?;
        }
        if finished {
            return Ok(());
        }
        if queue.state().stopping {
            continue;
        }
        if context.cancellation_requested() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "Diagnostic routing cancelled"));
        }
        context
            .sleep_until(
                context.now().checked_add(Duration::from_millis(100)).map_err(io::Error::other)?,
            )
            .await;
    }
}

mod batches;
#[cfg(test)]
mod tests;
