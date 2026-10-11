// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own scheduler-independent execution identities, admission and job lifetimes.
//!
//! Application services assign scope/job identities and surface generations.
//! The execution adapter owns scheduling, admission, actual task termination and
//! terminal observation. Cancellation requests do not establish that work stopped.

mod context;
mod errors;
mod identity;
mod lifetime;
mod memory;
mod outcome;
mod ports;
mod spec;

pub use context::{CancelControl, Clock, JobContext, WaitFuture};
pub use errors::{AdmissionError, ExecutionError};
pub use identity::{
    ExecutionId, Generation, IdentityError, JobId, Moment, ScopeId, SurfaceId, TimeError,
};
pub use lifetime::{JobControl, JobPhase, JobStatus, OwnedJob, Scope, ScopeControl, ScopeSnapshot};
pub use memory::{MemoryLease, MemoryRelease};
pub use outcome::{JobCompletion, JobOutcome, PanicCause, TerminalKind};
pub use ports::{
    CompletionBatch, CompletionNotification, CompletionWake, CpuStep, CpuTask, ExecutionPort,
};
pub use spec::{JobClass, JobSpec, Operation, ReplacementKey, ScopeSpec};
