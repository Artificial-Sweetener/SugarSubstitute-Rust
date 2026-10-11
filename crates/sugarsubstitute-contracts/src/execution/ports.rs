// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Bound generic task admission and typed terminal metadata without scheduler access.

use super::{
    AdmissionError, ExecutionError, JobContext, JobId, JobSpec, OwnedJob, Scope, ScopeId,
    ScopeSpec, TerminalKind,
};
use std::{future::Future, num::NonZeroUsize};

/// Finish a cooperative CPU step or yield to the execution owner's priority policy.
pub enum CpuStep<T> {
    /// Retain owned task state and yield before its next bounded step.
    Yield,
    /// Finish actual CPU work with its typed result.
    Complete(T),
}

/// Own resumable CPU work that observes cancellation between bounded steps.
///
/// Every step must return within its documented bound and avoid unbounded blocking.
/// Cancellation queries indicate requests; the adapter may report terminal
/// cancellation only after a running step actually returns. The adapter alone
/// chooses worker threads, priority, queue admission and subsequent step scheduling.
pub trait CpuTask<T, E>: Send + 'static {
    /// Execute one bounded cooperative step, retaining unfinished state in `self`.
    ///
    /// # Errors
    /// Returns the task's original typed domain error.
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<T>, E>;
}

/// Notify metadata consumers about one actually terminal typed job.
///
/// Draining this metadata does not acknowledge typed completion or release its
/// delivery credit. The job's owned handle transfers that completion separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletionNotification {
    /// Application scope that admitted the job.
    pub scope_id: ScopeId,
    /// Application job identity.
    pub job_id: JobId,
    /// Actual terminal outcome class.
    pub kind: TerminalKind,
    /// Accepted replacing job when the terminal kind is coalesced.
    pub replacement: Option<JobId>,
}

/// Return a bounded metadata drain with the pending count for another native wake.
#[derive(Debug)]
pub struct CompletionBatch {
    /// Terminal metadata in this drain, bounded by the requested count and 32.
    pub notifications: Vec<CompletionNotification>,
    /// Metadata still pending in the execution owner's bounded mailbox.
    pub remaining: usize,
}

/// Wake an actual native event loop after the execution owner publishes metadata.
///
/// The native adapter owns its thread-safe event-loop proxy. The execution adapter
/// owns the bounded mailbox and coalesces pending wakes. This port neither carries
/// job payloads nor exposes a channel, event loop or raw scheduler handle.
pub trait CompletionWake: Send + Sync {
    /// Request an observed native wake without blocking for UI consumption.
    ///
    /// # Errors
    /// Preserves an unexpected or closed native wake boundary's original cause.
    fn wake(&self) -> Result<(), ExecutionError>;
}

/// Admit generic owned work through the single authoritative execution adapter.
///
/// This intentionally generic trait does not require dynamic dispatch. Factories
/// run on an admitted execution worker after atomic queue, terminal-credit and
/// memory admission, never on the submitting UI thread. Rejected work and pending
/// cancellation/coalescing do not invoke their factory. Async work accepts only I/O
/// classes, blocking work only infrastructure classes, CPU tasks only CPU classes.
/// Accepted scope and job identities independently increase within an executor
/// lifetime in actual admission order. Rejected admission does not advance either
/// high-water mark; retired identities remain unavailable without unbounded ID sets.
/// Rejected admission consumes and drops its uninvoked factory on the caller.
/// Captures must therefore be cheap to drop or retain recoverable ownership outside
/// the factory. Accepted factory and untransferred-result retirement belongs to
/// execution workers and retains capacity until disposal actually returns.
pub trait ExecutionPort: Send + Sync {
    /// Create an application scope, inheriting parent cancellation and deadlines.
    ///
    /// # Errors
    /// Rejects closed admission, duplicate identities, expired deadlines, foreign
    /// parent scopes, nonadvancing identities, saturated scope capacity and
    /// unexpected adapter failures before creating the scope.
    fn create_scope(
        &self,
        spec: ScopeSpec,
        parent: Option<&Scope>,
    ) -> Result<Scope, AdmissionError>;

    /// Admit an async task factory into bounded ordinary/lifecycle I/O capacity.
    ///
    /// # Errors
    /// Returns a typed admission decision before invoking the factory.
    fn submit_async<T, E, F, M>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        factory: M,
    ) -> Result<OwnedJob<T, E>, AdmissionError>
    where
        T: Send + 'static,
        E: Send + 'static,
        F: Future<Output = Result<T, E>> + Send + 'static,
        M: FnOnce(JobContext) -> F + Send + 'static;

    /// Admit finite blocking work into bounded infrastructure capacity.
    ///
    /// Cancellation requests do not stop a started closure. The adapter retains
    /// observation until its bounded/cooperative operation actually returns.
    ///
    /// # Errors
    /// Returns a typed admission decision before invoking the closure.
    fn submit_blocking<T, E, M>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        factory: M,
    ) -> Result<OwnedJob<T, E>, AdmissionError>
    where
        T: Send + 'static,
        E: Send + 'static,
        M: FnOnce(JobContext) -> Result<T, E> + Send + 'static;

    /// Admit a resumable CPU task factory into interactive/decode capacity.
    ///
    /// # Errors
    /// Returns a typed admission decision before constructing the task.
    fn submit_cpu<T, E, C, M>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        factory: M,
    ) -> Result<OwnedJob<T, E>, AdmissionError>
    where
        T: Send + 'static,
        E: Send + 'static,
        C: CpuTask<T, E>,
        M: FnOnce(JobContext) -> C + Send + 'static;

    /// Drain terminal metadata without waiting or acknowledging typed delivery.
    ///
    /// The adapter clamps `max` to 32 and reports remaining metadata; it coalesces
    /// and reissues native wakes for pending batches. Consumers bound their own UI
    /// callback work and take typed outcomes through the corresponding owned job.
    ///
    /// # Errors
    /// Preserves unexpected mailbox/wake failures with their original cause.
    fn drain_completions(&self, max: NonZeroUsize) -> Result<CompletionBatch, ExecutionError>;
}
