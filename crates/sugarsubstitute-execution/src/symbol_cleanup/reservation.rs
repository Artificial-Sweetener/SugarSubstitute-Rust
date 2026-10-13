// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Construct held cleanup and its fixed source metadata inside the sole real admission.
use super::*;
use sugarsubstitute_contracts::execution::MemoryClaim;
impl ExecutionClient {
    /// Construct one closed source and held cleanup factory after real byte admission.
    /// The constructor receives exactly4096 bytes of the original32768 cleanup reservation.
    /// It must preflight its complete metadata/control layout, retain this claim outside
    /// its allocation, and perform only metadata construction: no clock/native callback,
    /// public job/control creation, or Core-reentrant destructor is permitted.
    /// Both returned owners share that same source; native work begins only in admitted
    /// factories. Cancellation and disposal use the existing held lifecycle owner.
    /// # Errors
    /// Rejects the same original scope/specification/capacity failures as ordinary held
    /// cleanup, returning the identical unexecuted constructor. Original claim failure
    /// also precedes constructor invocation and preserves it.
    pub fn reserve_symbol_cleanup_with_source<T, E, S, M, C>(
        &self,
        cleanup_scope: &Scope,
        cleanup_spec: JobSpec,
        main_scope: &Scope,
        main_spec: JobSpec,
        constructor: C,
    ) -> Result<(S, SymbolCleanupPermit<T, E>), SymbolSubmissionRejection<C>>
    where
        T: Send + 'static,
        E: Send + 'static,
        S: Send + 'static,
        M: FnOnce(JobContext) -> Result<T, E> + Send + 'static,
        C: FnOnce(MemoryClaim) -> (S, M) + Send + 'static,
    {
        self.reserve_symbol_cleanup_construct(
            cleanup_scope,
            cleanup_spec,
            main_scope,
            main_spec,
            constructor,
            |constructor, context| {
                let claim = match context.memory().claim(4096) {
                    Ok(claim) => claim,
                    Err(cause) => {
                        return Err((
                            AdmissionError::Unexpected(crate::runtime::error(cause)),
                            constructor,
                        ));
                    }
                };
                let (source, factory) = constructor(claim);
                Ok((factory, source))
            },
        )
    }
    pub(super) fn reserve_symbol_cleanup_construct<T, E, M, I, S>(
        &self,
        cleanup_scope: &Scope,
        cleanup_spec: JobSpec,
        main_scope: &Scope,
        main_spec: JobSpec,
        input: I,
        construct: impl FnOnce(I, &JobContext) -> Result<(M, S), (AdmissionError, I)>,
    ) -> Result<(S, SymbolCleanupPermit<T, E>), SymbolSubmissionRejection<I>>
    where
        T: Send + 'static,
        E: Send + 'static,
        M: FnOnce(JobContext) -> Result<T, E> + Send + 'static,
    {
        let main_owner = main_scope.owner_id();
        if main_owner != self.core.id {
            return Err(SymbolSubmissionRejection {
                error: AdmissionError::ForeignScope {
                    expected_owner: self.core.id,
                    actual_owner: main_owner,
                },
                factory: input,
            });
        }
        self.core
            .admit_owned(
                cleanup_scope,
                cleanup_spec,
                RequestKind::SymbolCleanup { main_scope: main_scope.spec(), main_spec },
                input,
                |input, context, cancel, timing| {
                    // Construct only metadata before any public owner that could re-enter Core.
                    let (factory, source) = construct(input, &context)?;
                    let job = Job::new(
                        cleanup_scope.id(),
                        cleanup_spec,
                        timing.admitted,
                        cancel,
                        Arc::downgrade(&self.core),
                    );
                    let task_job = job.clone();
                    let queued = QueuedJob {
                        context,
                        job: job.clone(),
                        work: Work::BlockingHeld(Box::new(move |context, started| {
                            Box::pin(crate::blocking::run(task_job, context, started, factory))
                        })),
                    };
                    let permit = SymbolCleanupPermit {
                        job: Box::new(OwnedJob::new(
                            cleanup_scope.id(),
                            cleanup_spec,
                            Box::new(JobHandle(job)),
                        )),
                        core: Arc::downgrade(&self.core),
                    };
                    Ok((queued, (source, permit)))
                },
            )
            .map_err(|(error, factory)| SymbolSubmissionRejection { error, factory })
    }
}
