// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Preserve an original input before admission and a returned source failure after stop.
use crate::{
    ExecutionClient,
    admission::RequestKind,
    task::{Job, JobHandle, QueuedJob, Work},
};
use std::{error::Error, fmt, sync::Arc};
use sugarsubstitute_contracts::execution::{
    AdmissionError, CpuTask, JobContext, JobSpec, MemoryClaim, OwnedJob, Scope,
};
/// Owned-input rejection. A rejected submission leaves the complete input unchanged.
#[derive(Debug)]
pub enum OwnedInputError {
    /// No original input was supplied; admission has not begun.
    MissingInput,
    /// Actual admission rejected the original input before ownership transfer.
    Admission(AdmissionError),
}
impl fmt::Display for OwnedInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingInput => f.write_str("Owned job input is absent"),
            Self::Admission(cause) => cause.fmt(f),
        }
    }
}
impl Error for OwnedInputError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MissingInput => None,
            Self::Admission(cause) => Some(cause),
        }
    }
}
/// Keep one actual admitted input box inseparable from its exact storage claim.
/// The box destroys and deallocates before the outside claim releases reusable bytes.
/// Borrowing preserves complete input custody; there is no bare extraction or claim split.
#[derive(Debug)]
pub struct ChargedBlockingInput<I> {
    value: Box<I>,
    _storage: MemoryClaim,
}
impl<I> ChargedBlockingInput<I> {
    /// Borrow the identical admitted input without transferring its charge.
    pub fn input(&self) -> &I {
        &self.value
    }
    /// Mutate the owned input while its original box remains funded.
    pub fn input_mut(&mut self) -> &mut I {
        &mut self.value
    }
}
impl ExecutionClient {
    /// Admit complete input without consuming it on rejection, preserving first returned failure.
    /// Factories own input only after actual admission. Returned errors retain their first
    /// source after post-worker stop observations; success still obeys the stop veto.
    /// Factories preserve secondary controls in their owned errors. This never waits.
    /// # Errors
    /// Missing input or actual rejection leaves the identical caller Option unchanged.
    pub fn submit_blocking_owned_input<T, E, I, M>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        input: &mut Option<I>,
        factory: M,
    ) -> Result<OwnedJob<T, E>, OwnedInputError>
    where
        T: Send + 'static,
        E: Send + 'static,
        I: Send + 'static,
        M: FnOnce(I, JobContext) -> Result<T, E> + Send + 'static,
    {
        self.submit_blocking_input(scope, spec, input, |input, _| take(input), factory)
    }
    /// Transfer exclusive reusable state into a cooperative CPU task after actual admission.
    /// Rejection leaves the identical input in the caller. Factories and task destruction run
    /// on execution workers; CPU class, replacement, cancellation and yield rules stay unchanged.
    /// This never waits and creates no scheduler or additional worker.
    /// # Errors
    /// Missing input or actual admission rejection preserves the complete original caller Option.
    pub fn submit_cpu_owned_input<T, E, I, C, M>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        input: &mut Option<I>,
        factory: M,
    ) -> Result<OwnedJob<T, E>, OwnedInputError>
    where
        T: Send + 'static,
        E: Send + 'static,
        I: Send + 'static,
        C: CpuTask<T, E>,
        M: FnOnce(I, JobContext) -> C + Send + 'static,
    {
        let mut pending = Some(factory);
        self.submit_input(scope, spec, RequestKind::Cpu, input, |input, job, _| {
            let value = take(input)?;
            let factory = pending.take().ok_or_else(missing)?;
            Ok(Work::Cpu(Box::new(move |context, started| {
                crate::cpu::work(job, context, started, move |context| factory(value, context))
            })))
        })
    }
    /// Fund the exact input box before allocation or transfer into the same finite worker.
    /// The constructor claims `size_of::<I>()` from the actual admitted reservation before
    /// taking the caller Option. Returned source failures retain their complete funded box
    /// across cancellation, terminal acknowledgement and worker joins. Successful output
    /// still obeys the stop veto. This creates no worker, extra reservation or native authority.
    /// # Errors
    /// Missing input, admission rejection or actual inline MemoryClaimError leaves the
    /// identical caller Option unchanged. No input box is allocated on claim failure.
    pub fn submit_blocking_owned_box<T, E, I, M>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        input: &mut Option<I>,
        factory: M,
    ) -> Result<OwnedJob<T, E>, OwnedInputError>
    where
        T: Send + 'static,
        E: Send + 'static,
        I: Send + 'static,
        M: FnOnce(ChargedBlockingInput<I>, JobContext) -> Result<T, E> + Send + 'static,
    {
        self.submit_blocking_input(
            scope,
            spec,
            input,
            |input, context| {
                let storage = context
                    .memory()
                    .claim(size_of::<I>() as u64)
                    .map_err(AdmissionError::Memory)?;
                Ok(ChargedBlockingInput { value: Box::new(take(input)?), _storage: storage })
            },
            factory,
        )
    }
    /// Preserve first returned failures for blocking work through the shared ownership transaction.
    fn submit_blocking_input<T, E, I, V, P, M>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        input: &mut Option<I>,
        prepare: P,
        factory: M,
    ) -> Result<OwnedJob<T, E>, OwnedInputError>
    where
        T: Send + 'static,
        E: Send + 'static,
        I: Send + 'static,
        V: Send + 'static,
        P: FnOnce(&mut Option<I>, &JobContext) -> Result<V, AdmissionError>,
        M: FnOnce(V, JobContext) -> Result<T, E> + Send + 'static,
    {
        let mut pending_factory = Some(factory);
        let mut pending_prepare = Some(prepare);
        self.submit_input(scope, spec, RequestKind::Blocking, input, |input, job, context| {
            let prepare = pending_prepare.take().ok_or_else(missing)?;
            let value = prepare(input, context)?;
            let factory = pending_factory.take().ok_or_else(missing)?;
            Ok(Work::Blocking(Box::new(move |context, started| {
                Box::pin(crate::blocking::run_source_first(job, context, started, move |context| {
                    factory(value, context)
                }))
            })))
        })
    }
    /// One queue/memory transaction for exclusive input; borrowed builders retain captures outside
    /// Core's mutex on rejection. Only successful construction may consume the original input.
    fn submit_input<T, E, I>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        kind: RequestKind<'_>,
        input: &mut Option<I>,
        mut construct: impl FnMut(
            &mut Option<I>,
            Arc<Job<T, E>>,
            &JobContext,
        ) -> Result<Work, AdmissionError>,
    ) -> Result<OwnedJob<T, E>, OwnedInputError>
    where
        T: Send + 'static,
        E: Send + 'static,
        I: Send + 'static,
    {
        if input.is_none() {
            return Err(OwnedInputError::MissingInput);
        }
        self.core
            .admit_owned(scope, spec, kind, input, |input, context, cancel, timing| {
                let job =
                    Job::new(scope.id(), spec, timing.admitted, cancel, Arc::downgrade(&self.core));
                let work =
                    construct(input, job.clone(), &context).map_err(|cause| (cause, input))?;
                let owner = OwnedJob::new(scope.id(), spec, Box::new(JobHandle(job.clone())));
                Ok((QueuedJob { context, job, work }, owner))
            })
            .map_err(|(cause, _input)| OwnedInputError::Admission(cause))
    }
}
fn missing() -> AdmissionError {
    AdmissionError::Unexpected(crate::runtime::error(OwnedInputError::MissingInput))
}
fn take<I>(input: &mut Option<I>) -> Result<I, AdmissionError> {
    input.take().ok_or_else(missing)
}

#[cfg(test)]
#[path = "blocking_box_tests.rs"]
mod box_tests;
#[cfg(test)]
#[path = "cpu_input_tests.rs"]
mod cpu_tests;
#[cfg(test)]
#[path = "blocking_input_tests.rs"]
mod tests;
