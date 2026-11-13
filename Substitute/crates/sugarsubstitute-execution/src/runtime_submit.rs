// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Construct typed task ownership only after the central admission transaction succeeds.

use super::*;

impl sugarsubstitute_contracts::execution::ExecutionPort for ExecutionClient {
    fn create_scope(
        &self,
        spec: ScopeSpec,
        parent: Option<&Scope>,
    ) -> Result<Scope, AdmissionError> {
        self.core.create_scope(spec, parent)
    }
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
        M: FnOnce(sugarsubstitute_contracts::execution::JobContext) -> F + Send + 'static,
    {
        self.submit(scope, spec, RequestKind::Async, |job| {
            Work::Async(Box::new(move |context, started| {
                Box::pin(task::run_async(job, context, started, factory))
            }))
        })
    }
    fn submit_blocking<T, E, M>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        factory: M,
    ) -> Result<OwnedJob<T, E>, AdmissionError>
    where
        T: Send + 'static,
        E: Send + 'static,
        M: FnOnce(sugarsubstitute_contracts::execution::JobContext) -> Result<T, E>
            + Send
            + 'static,
    {
        self.submit(scope, spec, RequestKind::Blocking, |job| {
            Work::Blocking(Box::new(move |context, started| {
                Box::pin(crate::blocking::run(job, context, started, factory))
            }))
        })
    }
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
        M: FnOnce(sugarsubstitute_contracts::execution::JobContext) -> C + Send + 'static,
    {
        self.submit(scope, spec, RequestKind::Cpu, |job| {
            Work::Cpu(Box::new(move |context, started| {
                crate::cpu::work(job, context, started, factory)
            }))
        })
    }
    fn drain_completions(&self, maximum: NonZeroUsize) -> Result<CompletionBatch, ExecutionError> {
        self.core.mailbox.drain(maximum)
    }
}

impl ExecutionClient {
    #[cfg(test)]
    pub(crate) fn core_for_test(&self) -> Arc<Core> {
        self.core.clone()
    }
    fn submit<T: Send + 'static, E: Send + 'static>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        kind: RequestKind<'_>,
        work: impl FnOnce(Arc<Job<T, E>>) -> Work,
    ) -> Result<OwnedJob<T, E>, AdmissionError> {
        self.core.admit(scope, spec, kind, |context, cancel, timing| {
            let admitted = timing.admitted;
            let job = Job::new(scope.id(), spec, admitted, cancel, Arc::downgrade(&self.core));
            let queued = QueuedJob { context, job: job.clone(), work: work(job.clone()) };
            (queued, OwnedJob::new(scope.id(), spec, Box::new(JobHandle(job))))
        })
    }
}
