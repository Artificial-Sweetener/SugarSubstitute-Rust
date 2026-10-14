// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Construct held cleanup and its fixed source metadata inside the sole real admission.
use super::*;
impl ExecutionClient {
    pub(super) fn reserve_report_export_cleanup_construct<T, E, M, I, S>(
        &self,
        cleanup_scope: &Scope,
        cleanup_spec: JobSpec,
        main_scope: &Scope,
        main_spec: JobSpec,
        input: I,
        construct: impl FnOnce(I, &JobContext) -> Result<(M, S), (AdmissionError, I)>,
    ) -> Result<(S, ReportExportCleanupPermit<T, E>), ReportExportSubmissionRejection<I>>
    where
        T: Send + 'static,
        E: Send + 'static,
        M: FnOnce(JobContext) -> Result<T, E> + Send + 'static,
    {
        let main_owner = main_scope.owner_id();
        if main_owner != self.core.id {
            return Err(ReportExportSubmissionRejection {
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
                RequestKind::ReportExportCleanup { main_scope: main_scope.spec(), main_spec },
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
                    let permit = ReportExportCleanupPermit {
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
            .map_err(|(error, factory)| ReportExportSubmissionRejection { error, factory })
    }
}
