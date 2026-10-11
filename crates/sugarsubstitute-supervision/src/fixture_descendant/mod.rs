// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Create only a same-executable held descendant in its parent's existing Job chain.
//! No new Job, breakaway, arbitrary target, signal, wait or process termination exists
//! here. The outer registry retains authoritative containment and actual family exit.
mod input;
#[allow(unsafe_code)]
mod native;
mod native_checks;
use crate::{RegistryAdmissionError, RegistryError};
pub use input::FixtureDescendantInput;
use std::sync::{Arc, Mutex, MutexGuard};
use sugarsubstitute_contracts::execution::{
    ExecutionPort, JobCompletion, JobContext, JobId, JobSpec, MemoryClaim, OwnedJob, Scope,
};
use sugarsubstitute_execution::{ExecutionClient, windows::NativeCorrelation};

/// Actual identity after successful suspended creation, containment query and resume.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixtureDescendantStarted {
    /// Original campaign correlation; no authentication authority.
    pub correlation: NativeCorrelation,
    /// Actual kernel PID, not an application logical identity.
    pub process_id: u32,
    /// Actual kernel creation FILETIME in100ns units.
    pub creation_time_100ns: u64,
}
trait Lease: Send {
    fn create(
        &mut self,
        input: &mut FixtureDescendantInput,
        context: &JobContext,
    ) -> Result<FixtureDescendantStarted, RegistryError>;
    fn retire(&mut self) -> Result<(), RegistryError>;
}
struct NativeState {
    lease: Option<Box<dyn Lease>>,
    claim: Option<MemoryClaim>,
    started: Option<FixtureDescendantStarted>,
    retired: bool,
}
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
impl Drop for NativeState {
    fn drop(&mut self) {
        if !self.retired
            && self.claim.is_some()
            && let Some(lease) = self.lease.take()
        {
            // Closing private native outputs on a caller is forbidden. Failed final
            // ownership remains until isolated process death; the outer Job still
            // contains every created descendant. This is never a clean receipt.
            tracing::error!(
                operation = "fixture_descendant_unretired",
                "Unretired fixed fixture native ownership retained"
            );
            std::mem::forget(lease);
            if let Some(claim) = self.claim.take() {
                std::mem::forget(claim);
            }
        }
    }
}
/// Retain partial native creation and checked closure through explicit finite jobs.
/// Consume completions and retire before execution shutdown. Drop never closes a
/// native handle on the caller and cannot establish descendant/family settlement.
#[must_use]
pub struct FixtureDescendantLaunch {
    shared: Arc<Mutex<NativeState>>,
    job: Option<OwnedJob<(), RegistryError>>,
}
impl FixtureDescendantLaunch {
    /// Admit finite creation before native effects. Only the fixed input is accepted.
    pub fn submit(
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        input: FixtureDescendantInput,
    ) -> Result<Self, RegistryAdmissionError> {
        Self::submit_with(execution, scope, spec, input, Box::new(native::NativeLease::new()))
    }
    fn submit_with(
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        input: FixtureDescendantInput,
        lease: Box<dyn Lease>,
    ) -> Result<Self, RegistryAdmissionError> {
        crate::registry::validate_blocking(spec)?;
        if spec.reservation_bytes < input.required_reservation_bytes() || spec.deadline.is_none() {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        let shared = Arc::new(Mutex::new(NativeState {
            lease: Some(lease),
            claim: None,
            started: None,
            retired: false,
        }));
        let worker = shared.clone();
        let job = execution
            .submit_blocking(scope, spec, move |context| create(worker, input, context))
            .map_err(RegistryAdmissionError::Execution)?;
        Ok(Self { shared, job: Some(job) })
    }
    /// Transfer actual finite terminal delivery; no private-state lock is held while work runs.
    pub fn take_completion(&mut self) -> Option<JobCompletion<(), RegistryError>> {
        let completion = self.job.as_mut()?.try_take_completion()?;
        self.job = None;
        Some(completion)
    }
    /// Transfer the actual successful start once, after the finite creation terminal was taken.
    pub fn take_started(&mut self) -> Option<FixtureDescendantStarted> {
        if self.job.is_some() {
            return None;
        }
        lock(&self.shared).started.take()
    }
    /// True only after every owned output and attribute list actually retired.
    /// A running finite job reports false without blocking on its native work.
    pub fn native_retired(&self) -> bool {
        self.job.is_none() && lock(&self.shared).retired
    }
    /// Admit one explicit finite closure retry. Failure retains state; no hidden retry timer.
    pub fn submit_retirement(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<JobId, RegistryAdmissionError> {
        crate::registry::validate_blocking(spec)?;
        if spec.reservation_bytes < 4096 {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        if self.job.is_some() {
            return Err(RegistryAdmissionError::Busy);
        }
        if self.native_retired() {
            return Err(RegistryAdmissionError::Closed);
        }
        let shared = self.shared.clone();
        self.job = Some(
            execution
                .submit_blocking(scope, spec, move |_context| retire(&mut lock(&shared)))
                .map_err(RegistryAdmissionError::Execution)?,
        );
        Ok(spec.id)
    }
}
fn create(
    shared: Arc<Mutex<NativeState>>,
    mut input: FixtureDescendantInput,
    context: JobContext,
) -> Result<(), RegistryError> {
    let mut state = lock(&shared);
    state.claim = Some(
        context
            .memory()
            .claim(input.required_reservation_bytes())
            .map_err(|cause| RegistryError::new("fixture_descendant_memory", cause))?,
    );
    let result = if context.cancellation_requested()
        || context.deadline().is_some_and(|deadline| context.now() >= deadline)
    {
        Err(RegistryError::invalid("fixture_descendant_canceled"))
    } else {
        state
            .lease
            .as_mut()
            .ok_or_else(|| RegistryError::invalid("fixture_descendant_owner"))?
            .create(&mut input, &context)
    };
    if let Ok(started) = &result {
        state.started = Some(*started);
    }
    let cleanup = retire(&mut state);
    match (result, cleanup) {
        (Err(mut cause), Err(cleanup)) => {
            cause.retain(cleanup);
            Err(cause)
        }
        (Err(cause), _) => Err(cause),
        (Ok(_), cleanup) => cleanup,
    }
}
fn retire(state: &mut NativeState) -> Result<(), RegistryError> {
    if let Some(lease) = &mut state.lease {
        lease.retire()?;
    }
    drop(state.lease.take());
    state.claim = None;
    state.retired = true;
    Ok(())
}
#[cfg(test)]
mod cancellation_tests;
#[cfg(test)]
mod tests;
