// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Provide job identity, cancellation, clock and retained memory without scheduling access.

use super::{
    ExecutionId, ForkJoin, ForkJoinItem, JobId, JobSpec, MemoryLease, Moment, NativeHandleLease,
    NativeHandleRelease, ScopeId,
};
use std::{future::Future, pin::Pin, sync::Arc};

/// Own a scheduler-neutral wait without exposing a runtime, channel or raw waker.
pub type WaitFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// Supply the execution owner's injected monotonic clock and bounded timer waits.
///
/// Port calls are nonblocking and must not panic. The execution owner samples
/// supplied clocks outside its internal admission and lifetime locks.
pub trait Clock: Send + Sync {
    /// Return nanoseconds in this clock's monotonic epoch.
    fn now(&self) -> Moment;

    /// Own a wait through the requested moment; elapsed deadlines complete promptly.
    ///
    /// The future owns its registration. Dropping it releases that registration;
    /// it does not establish termination of the job that requested the wait.
    fn sleep_until(&self, deadline: Moment) -> WaitFuture;
}

/// Observe an adapter-owned cancellation request independently of task termination.
pub trait CancelControl: Send + Sync {
    /// Report whether a cancellation request has been recorded.
    fn is_requested(&self) -> bool;

    /// Own a wait that completes when cancellation has been requested.
    ///
    /// Completion signals the request, not that native/blocking work has stopped.
    fn cancelled(&self) -> WaitFuture;
}

/// Transfer byte and optional fixed symbol accounting already accepted by execution admission.
/// Only the actual admission owner supplies a release port; ordinary contexts supply `None`.
pub struct AdmittedJobResources {
    memory: MemoryLease,
    symbol_release: Option<Box<dyn NativeHandleRelease>>,
    fork_join: Option<Arc<dyn ForkJoin>>,
}
impl AdmittedJobResources {
    /// Bundle already-admitted byte ownership and its optional fixed symbol reservation release.
    /// This constructs no queue, memory, native-output credit or native authority.
    pub fn new(memory: MemoryLease, symbol_release: Option<Box<dyn NativeHandleRelease>>) -> Self {
        Self { memory, symbol_release, fork_join: None }
    }
    /// Let the job spread borrowed work over the execution owner's bounded helper capacity.
    #[must_use]
    pub fn with_fork_join(mut self, fork_join: Arc<dyn ForkJoin>) -> Self {
        self.fork_join = Some(fork_join);
        self
    }
}

/// Own the bounded capabilities available to one admitted job.
///
/// Clones retain cancellation/clock ports and the same byte charge. Contexts expose
/// no submission, runtime, channel, native event-loop or worker-pool handles. CPU contexts may
/// spread borrowed work through a bounded fork-join that completes before it returns.
#[derive(Clone)]
pub struct JobContext {
    owner_id: ExecutionId,
    scope_id: ScopeId,
    spec: JobSpec,
    deadline: Option<Moment>,
    ports: Arc<JobPorts>,
    clock: Arc<dyn Clock>,
    memory: MemoryLease,
    native_handles: Option<NativeHandleLease>,
}

/// Per-job adapter ports behind one thin handle; contexts also live in fixed native storage.
struct JobPorts {
    cancellation: Arc<dyn CancelControl>,
    fork_join: Option<Arc<dyn ForkJoin>>,
}

impl JobContext {
    /// Construct admitted context from real execution-owned capability ports.
    ///
    /// The adapter computes `deadline` from the job and all ancestor scopes and
    /// reserves `memory` before constructing the task on an execution worker.
    pub fn new(
        owner_id: ExecutionId,
        scope_id: ScopeId,
        spec: JobSpec,
        deadline: Option<Moment>,
        cancellation: Arc<dyn CancelControl>,
        clock: Arc<dyn Clock>,
        resources: AdmittedJobResources,
    ) -> Self {
        let native_handles = resources
            .symbol_release
            .map(|release| NativeHandleLease::new(owner_id, scope_id, spec.id, release));
        Self {
            owner_id,
            scope_id,
            spec,
            deadline,
            ports: Arc::new(JobPorts { cancellation, fork_join: resources.fork_join }),
            clock,
            memory: resources.memory,
            native_handles,
        }
    }

    /// Return the executor identity owning this job and its clock epoch.
    pub const fn owner_id(&self) -> ExecutionId {
        self.owner_id
    }

    /// Return the application scope that admitted this job.
    pub const fn scope_id(&self) -> ScopeId {
        self.scope_id
    }

    /// Return the application-assigned job identity.
    pub const fn job_id(&self) -> JobId {
        self.spec.id
    }

    /// Return the original admitted specification.
    pub const fn spec(&self) -> JobSpec {
        self.spec
    }

    /// Return the effective deadline bounded by the job and all ancestor scopes.
    pub const fn deadline(&self) -> Option<Moment> {
        self.deadline
    }

    /// Narrow this already-admitted job to one finite operation in the same clock epoch.
    ///
    /// The effective deadline is the earlier of this bound and every original job/scope bound.
    /// Identity, original specification, cancellation, native custody and memory admission remain
    /// shared. This creates no job or scheduler access and cannot extend an existing deadline.
    /// Storage services may keep their command lifetime while bounding each individual effect.
    pub fn bounded_until(&self, deadline: Moment) -> Self {
        let mut bounded = self.clone();
        bounded.deadline = Some(self.deadline.map_or(deadline, |original| original.min(deadline)));
        bounded
    }

    /// Observe a cancellation request without claiming the job has stopped.
    pub fn cancellation_requested(&self) -> bool {
        self.ports.cancellation.is_requested()
    }

    /// Own a wait for cancellation request notification.
    pub fn cancelled(&self) -> WaitFuture {
        self.ports.cancellation.cancelled()
    }

    /// Return the current moment in this job's injected monotonic clock.
    pub fn now(&self) -> Moment {
        self.clock.now()
    }

    /// Own a clock wait without borrowing this context or exposing its scheduler.
    pub fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        self.clock.sleep_until(deadline)
    }

    /// Borrow the shared memory charge; clone it when result buffers outlive context.
    pub fn memory(&self) -> &MemoryLease {
        &self.memory
    }
    /// Borrow original fixed symbol output accounting; ordinary jobs return `None`.
    pub fn native_handles(&self) -> Option<&NativeHandleLease> {
        self.native_handles.as_ref()
    }

    /// Run `work` on every contiguous `chunk_len`-element chunk of `data` with its chunk index.
    ///
    /// CPU jobs spread groups of chunks over the execution owner's bounded helper threads and
    /// return only after every chunk finished; other contexts run chunks in order on the caller.
    /// Work observes this context's cancellation and deadline itself. A zero `chunk_len` is
    /// treated as one.
    /// # Errors
    /// Returns the error of the earliest failing group; later chunks in that group do not run.
    pub fn try_for_each_chunk_mut<T, E, F>(
        &self,
        data: &mut [T],
        chunk_len: usize,
        work: F,
    ) -> Result<(), E>
    where
        T: Send,
        E: Send,
        F: Fn(usize, &mut [T]) -> Result<(), E> + Sync,
    {
        let chunk_len = chunk_len.max(1);
        let chunks = data.len().div_ceil(chunk_len);
        let fork_join = self.ports.fork_join.as_ref().filter(|port| port.width() > 1 && chunks > 1);
        let Some(fork_join) = fork_join else {
            return data
                .chunks_mut(chunk_len)
                .enumerate()
                .try_for_each(|(index, chunk)| work(index, chunk));
        };
        let per_group = chunks.div_ceil(fork_join.width().min(chunks));
        let groups = chunks.div_ceil(per_group);
        let mut outcomes: Vec<Result<(), E>> = (0..groups).map(|_| Ok(())).collect();
        let work = &work;
        let items = data
            .chunks_mut(per_group * chunk_len)
            .zip(outcomes.iter_mut())
            .enumerate()
            .map(|(group, (slice, outcome))| {
                Box::new(move || {
                    *outcome = slice
                        .chunks_mut(chunk_len)
                        .enumerate()
                        .try_for_each(|(index, chunk)| work(group * per_group + index, chunk));
                }) as ForkJoinItem<'_>
            })
            .collect();
        fork_join.join(items);
        outcomes.into_iter().collect()
    }
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
