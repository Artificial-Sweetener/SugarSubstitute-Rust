// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Couple application scope/job handles to real adapter cancellation and observation.

use super::{ExecutionId, JobCompletion, JobSpec, ScopeId, ScopeSpec, TerminalKind};

/// Observe a scope without conflating cancellation requests with actual termination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScopeSnapshot {
    /// Application scope identity.
    pub id: ScopeId,
    /// Admission into this scope is closed.
    pub closed: bool,
    /// Cancellation has been requested for this scope.
    pub cancellation_requested: bool,
    /// Admitted jobs awaiting execution or worker-owned factory retirement.
    pub queued: usize,
    /// Jobs whose actual execution has not yet stopped.
    pub running: usize,
    /// Terminal jobs whose typed completion has not been acknowledged.
    pub terminal_unobserved: usize,
    /// Child scopes whose real lifetimes remain live.
    pub children_live: usize,
    /// Shared byte charges retained by jobs, completions or result guards.
    pub reserved_bytes: u64,
}

/// Own a real scope's executor identity, cancellation and abandonment boundary.
///
/// Methods are infallible, nonblocking and must not panic. Abandonment requests
/// cancellation and closes admission; the adapter retains all actual job lifetimes,
/// native joins and terminal observation until those jobs stop.
pub trait ScopeControl: Send + Sync {
    /// Identify the executor that owns this actual scope.
    fn owner_id(&self) -> ExecutionId;
    /// Request cancellation without claiming that the scope's jobs stopped.
    fn cancel(&self);
    /// Return the scope's current admission/execution/retention state.
    fn snapshot(&self) -> ScopeSnapshot;
    /// Abandon application ownership, request cancellation and retain real lifetimes.
    fn abandon(&self);
}

/// Own one application scope handle; dropping it abandons through the real adapter.
#[must_use = "Retain the scope for its application/task lifetime"]
pub struct Scope {
    spec: ScopeSpec,
    control: Box<dyn ScopeControl>,
}

impl Scope {
    /// Couple an accepted specification to its real execution-owned control.
    pub fn new(spec: ScopeSpec, control: Box<dyn ScopeControl>) -> Self {
        Self { spec, control }
    }
    /// Return the executor lifetime owning this scope.
    pub fn owner_id(&self) -> ExecutionId {
        self.control.owner_id()
    }
    /// Return the application-assigned scope identity.
    pub const fn id(&self) -> ScopeId {
        self.spec.id
    }
    /// Return the accepted scope specification.
    pub const fn spec(&self) -> ScopeSpec {
        self.spec
    }
    /// Request cancellation through the actual adapter; this does not wait for stop.
    pub fn cancel(&self) {
        self.control.cancel();
    }
    /// Observe actual queued/running/terminal and retained-memory state.
    pub fn snapshot(&self) -> ScopeSnapshot {
        self.control.snapshot()
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        self.control.abandon();
    }
}

/// Describe actual job execution independently of cancellation-request state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobPhase {
    /// Accepted work has not started its factory.
    Queued,
    /// Actual job execution has not stopped.
    Running,
    /// The adapter observed real termination with this outcome class.
    Terminal(TerminalKind),
}

/// Observe a job's actual phase and a separate cancellation-request flag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobStatus {
    /// Actual admitted execution phase.
    pub phase: JobPhase,
    /// Cancellation was requested; running blocking work may still be active.
    pub cancellation_requested: bool,
}

/// Own a real typed job's cancellation, terminal transfer and abandonment boundary.
///
/// Methods are nonblocking, infallible and must not panic. A completion is returned
/// at most once, after actual termination. Its transfer acknowledges delivery
/// credit; its memory guard independently retains the admitted byte charge.
/// Cancellation and abandonment after transfer are no-ops for this job handle;
/// a retained context still observes cancellation of its authoritative scope.
pub trait JobControl<T, E>: Send + Sync {
    /// Request cancellation while retaining observation of actual termination.
    fn cancel(&self);
    /// Observe actual execution phase and separate cancellation-request state.
    fn status(&self) -> JobStatus;
    /// Transfer and acknowledge a terminal completion once, or return no result.
    fn try_take_completion(&self) -> Option<JobCompletion<T, E>>;
    /// Request cancellation and abandon delivery while retaining joins/observation.
    fn abandon(&self);
}

/// Own one typed job handle without cloning or exposing a raw scheduler task.
#[must_use = "Observe the job's terminal result or explicitly abandon its lifetime"]
pub struct OwnedJob<T, E> {
    scope_id: ScopeId,
    spec: JobSpec,
    control: Box<dyn JobControl<T, E>>,
}

impl<T, E> OwnedJob<T, E> {
    /// Couple admitted job identity to its actual typed adapter control.
    pub fn new(scope_id: ScopeId, spec: JobSpec, control: Box<dyn JobControl<T, E>>) -> Self {
        Self { scope_id, spec, control }
    }
    /// Return the application scope that admitted the job.
    pub const fn scope_id(&self) -> ScopeId {
        self.scope_id
    }
    /// Return the exact admitted specification, including application job identity.
    pub const fn spec(&self) -> JobSpec {
        self.spec
    }
    /// Request cancellation; this is not proof of blocking/native termination.
    /// A job whose terminal completion has already transferred is unaffected.
    pub fn cancel(&self) {
        self.control.cancel();
    }
    /// Observe actual phase separately from a cancellation request.
    pub fn status(&self) -> JobStatus {
        self.control.status()
    }
    /// Transfer the terminal result once without waiting on the caller's thread.
    ///
    /// Taking the result acknowledges terminal-delivery credit. Keep its memory
    /// guard with retained buffers after transfer; draining metadata does not ack.
    pub fn try_take_completion(&mut self) -> Option<JobCompletion<T, E>> {
        self.control.try_take_completion()
    }
}

impl<T, E> Drop for OwnedJob<T, E> {
    fn drop(&mut self) {
        self.control.abandon();
    }
}

#[cfg(test)]
#[path = "lifetime_tests.rs"]
mod tests;
