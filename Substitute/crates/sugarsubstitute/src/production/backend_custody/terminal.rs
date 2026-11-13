// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain original helper terminal guards inline while classifying checked retirement.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{backend_custody::BackendCustodyKey, execution::*};
use sugarsubstitute_execution::{
    backend_custody::{
        BackendCustodyPipeReport, BackendCustodyWorkerFailure, BackendCustodyWorkerReport,
    },
    windows_lifecycle_pipe::LifecyclePipeFailure,
};

/// Immutable caller facts cached only after the original reservation succeeds.
#[derive(Clone, Copy)]
pub(super) struct Original {
    scope: ScopeId,
    spec: JobSpec,
}
impl Original {
    pub(super) fn new(scope: &Scope, spec: JobSpec) -> Self {
        Self { scope: scope.id(), spec }
    }
    fn matches<T, E>(&self, done: &JobCompletion<T, E>) -> bool {
        done.scope_id == self.scope
            && done.spec == self.spec
            && done.memory.bytes() == self.spec.reservation_bytes
            && done.native_handles.is_none()
    }
}

/// These inline slots allocate no wrapper/control and retain each original completion charge.
#[derive(Default)]
pub(super) struct Terminals {
    /// Original admitted pipe scope/spec; no later clock or identity replaces these facts.
    pub(super) pipe_original: Option<Original>,
    /// Original admitted worker scope/spec, shared by bound and unbound cancellation paths.
    pub(super) worker_original: Option<Original>,
    pipe: Option<JobCompletion<BackendCustodyPipeReport, LifecyclePipeFailure>>,
    worker: Option<JobCompletion<BackendCustodyWorkerReport, BackendCustodyWorkerFailure>>,
}
#[derive(Debug)]
enum Issue {
    Identity,
    Unexpected(TerminalKind),
}
impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity => f.write_str("Original helper terminal identity rejected"),
            Self::Unexpected(kind) => write!(f, "Original helper retirement returned {kind:?}"),
        }
    }
}
impl Error for Issue {}

/// Clone only the existing bounded error value; its unchanged full completion remains inline.
fn classify<T, E: Error + Clone + 'static>(
    outcome: &JobOutcome<T, E>,
    closing: bool,
) -> Result<(), Box<dyn Error>> {
    match outcome {
        JobOutcome::Success(_) => Ok(()),
        JobOutcome::Cancelled if closing => Ok(()),
        JobOutcome::Failure(cause) => Err(Box::new(cause.clone())),
        other => Err(Box::new(Issue::Unexpected(other.kind()))),
    }
}
impl Terminals {
    /// Observe once after the real pipe owner returns, retaining both guards and native cause.
    pub(super) fn pipe(
        &mut self,
        done: JobCompletion<BackendCustodyPipeReport, LifecyclePipeFailure>,
        closing: bool,
        authenticated: bool,
        key: BackendCustodyKey,
    ) -> Result<(), Box<dyn Error>> {
        let valid = self.pipe_original.is_some_and(|original| original.matches(&done))
            && match &done.outcome {
                JobOutcome::Success(report) => {
                    report.key == Some(key) || (!authenticated && report.key.is_none())
                }
                _ => true,
            };
        self.pipe = Some(done);
        if !valid {
            return Err(Box::new(Issue::Identity));
        }
        classify(
            &self.pipe.as_ref().ok_or("Original helper pipe terminal missing")?.outcome,
            closing,
        )
    }
    /// Observe the original worker's exact scope/spec/key without dropping its metadata guard.
    pub(super) fn worker(
        &mut self,
        done: JobCompletion<BackendCustodyWorkerReport, BackendCustodyWorkerFailure>,
        closing: bool,
        key: BackendCustodyKey,
    ) -> Result<(), Box<dyn Error>> {
        let valid = self.worker_original.is_some_and(|original| original.matches(&done))
            && match &done.outcome {
                JobOutcome::Success(report) => report.key() == key,
                _ => true,
            };
        self.worker = Some(done);
        if !valid {
            return Err(Box::new(Issue::Identity));
        }
        classify(
            &self.worker.as_ref().ok_or("Original helper worker terminal missing")?.outcome,
            closing,
        )
    }
}

#[cfg(test)]
#[path = "terminal/tests.rs"]
mod tests;
