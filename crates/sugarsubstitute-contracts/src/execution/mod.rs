// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own scheduler-independent execution identities, admission and job lifetimes.
//!
//! Application services assign scope/job identities and surface generations.
//! The execution adapter owns scheduling, admission, actual task termination and
//! terminal observation. Cancellation requests do not establish that work stopped.

mod context;
mod counter_deadline;
mod errors;
mod fork_join;
mod identity;
mod lifetime;
mod memory;
mod native_handles;
mod outcome;
mod ports;
mod spec;

pub use context::{AdmittedJobResources, CancelControl, Clock, JobContext, WaitFuture};
pub use counter_deadline::{CounterDeadline, CounterDeadlineError};
pub use errors::{AdmissionError, ExecutionError};
pub use fork_join::{ForkJoin, ForkJoinItem};
pub use identity::{
    ExecutionId, Generation, IdentityError, JobId, Moment, ScopeId, SurfaceId, TimeError,
};
pub use lifetime::{JobControl, JobPhase, JobStatus, OwnedJob, Scope, ScopeControl, ScopeSnapshot};
pub use memory::{
    MemoryClaim, MemoryClaimError, MemoryLease, MemoryLeaseObservation, MemoryRelease,
};
pub use native_handles::{
    NativeHandleClaim, NativeHandleLease, NativeHandleObservation, NativeHandleOutput,
    NativeHandleRelease, NativeResourceError, SymbolNativePartition,
};
pub use outcome::{JobCompletion, JobOutcome, PanicCause, TerminalKind};
pub use ports::{
    CompletionBatch, CompletionNotification, CompletionWake, CpuStep, CpuTask, ExecutionPort,
};
pub use spec::{JobClass, JobSpec, Operation, ReplacementKey, ScopeSpec};
