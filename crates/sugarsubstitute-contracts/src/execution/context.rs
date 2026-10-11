// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Provide job identity, cancellation, clock and retained memory without scheduling access.

use super::{ExecutionId, JobId, JobSpec, MemoryLease, Moment, ScopeId};
use std::{future::Future, pin::Pin, sync::Arc};

/// Own a scheduler-neutral wait without exposing a runtime, channel or raw waker.
pub type WaitFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// Supply the execution owner's injected monotonic clock and bounded timer waits.
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

/// Own the bounded capabilities available to one admitted job.
///
/// Clones retain cancellation/clock ports and the same byte charge. Contexts expose
/// no submission, runtime, channel, native event-loop or worker-pool handles.
#[derive(Clone)]
pub struct JobContext {
    owner_id: ExecutionId,
    scope_id: ScopeId,
    spec: JobSpec,
    deadline: Option<Moment>,
    cancellation: Arc<dyn CancelControl>,
    clock: Arc<dyn Clock>,
    memory: MemoryLease,
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
        memory: MemoryLease,
    ) -> Self {
        Self { owner_id, scope_id, spec, deadline, cancellation, clock, memory }
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

    /// Observe a cancellation request without claiming the job has stopped.
    pub fn cancellation_requested(&self) -> bool {
        self.cancellation.is_requested()
    }

    /// Own a wait for cancellation request notification.
    pub fn cancelled(&self) -> WaitFuture {
        self.cancellation.cancelled()
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
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
