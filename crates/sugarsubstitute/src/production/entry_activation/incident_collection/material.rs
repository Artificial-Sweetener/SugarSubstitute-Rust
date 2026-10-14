// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual failed-exit facts and UTC preparation under one finite original admission.
use super::*;
use std::time::{SystemTime, UNIX_EPOCH};
use sugarsubstitute_contracts::execution::*;
/// Metadata is inseparable from the original32KiB job claim and matching completion.
pub(super) struct FailureMaterial {
    pub(super) fact: FailedDesktopFact,
    pub(super) context: IncidentRequestContext,
    _storage: MemoryClaim,
}
/// Preserve the original conversion/control/allocation cause and complete finite charge.
#[derive(Debug)]
pub(super) struct FailureMaterialError {
    cause: Box<dyn Error + Send + Sync>,
    _original: MemoryLease,
}
impl std::fmt::Display for FailureMaterialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Failed desktop metadata preparation stopped")
    }
}
impl Error for FailureMaterialError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
/// Actual original preparation job, terminal notice and unchanged completion/cause.
pub(super) struct FailurePreparation {
    job: Option<OwnedJob<FailureMaterial, FailureMaterialError>>,
    completion: Option<JobCompletion<FailureMaterial, FailureMaterialError>>,
    notice: Option<TerminalKind>,
    admission: Option<AdmissionError>,
}
impl FailurePreparation {
    /// Construct without facts, admission or native effects.
    pub(super) const fn new() -> Self {
        Self { job: None, completion: None, notice: None, admission: None }
    }
    /// Submit exact retained native fact at its first unchanged failure cutoff.
    pub(super) fn submit(
        &mut self,
        fact: FailedDesktopFact,
        original: IncidentOperationBudget,
        resources: &EntryResources<'_>,
    ) -> Result<()> {
        if self.job.is_some() || self.completion.is_some() {
            return Ok(());
        }
        let spec = JobSpec {
            id: resources.jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(original.until()),
            reservation_bytes: 32 * 1024,
            replacement: None,
        };
        match resources.client.submit_blocking(resources.finite, spec, move |context| {
            prepare(fact, original, &context, || {
                let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?;
                Ok(IncidentUtc::from_unix_seconds(i64::try_from(elapsed.as_secs())?)?)
            })
        }) {
            Ok(job) => {
                self.job = Some(job);
                Ok(())
            }
            Err(cause) => {
                self.admission.get_or_insert(cause);
                Ok(())
            }
        }
    }
    /// Match notices from the one existing global batch; never drain central completions here.
    pub(super) fn capture(&mut self, batch: &CompletionBatch) -> Result<()> {
        let Some(job) = &mut self.job else {
            return Ok(());
        };
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) != (job.scope_id(), job.spec().id) {
                continue;
            }
            if self.notice.is_some_and(|old| old != notice.kind) {
                return Err("Failure material notice changed".into());
            }
            self.notice = Some(notice.kind);
        }
        if let Some(notice) = self.notice
            && let Some(done) = job.try_take_completion()
        {
            let valid = done.scope_id == job.scope_id()
                && done.spec == job.spec()
                && done.outcome.kind() == notice;
            self.completion = Some(done);
            self.job = None;
            if !valid {
                return Err("Failure material original completion changed".into());
            }
        }
        Ok(())
    }
    /// Borrow genuine prepared material; unsuccessful outcomes stay in their original completion.
    pub(super) fn material(&self) -> Result<Option<&FailureMaterial>> {
        match self.completion.as_ref().map(|done| &done.outcome) {
            Some(JobOutcome::Success(value)) => Ok(Some(value)),
            Some(_) => Err("Actual failure material stopped with retained original outcome".into()),
            None => Ok(None),
        }
    }
    /// Borrow original admission or executed failure independently from current retry state.
    pub(super) fn failure(&self) -> Option<&(dyn Error + 'static)> {
        match self.completion.as_ref().map(|done| &done.outcome) {
            Some(JobOutcome::Failure(cause)) => Some(cause),
            _ => self.admission.as_ref().map(|cause| cause as _),
        }
    }
    /// Request actual preparation cancellation while keeping its notice/completion lifetime.
    pub(super) fn cancel(&self) {
        if let Some(job) = &self.job {
            job.cancel();
        }
    }
    /// Observe absence of a live actual job only after matching terminal adoption or no admission.
    pub(super) fn delivered(&self) -> bool {
        self.job.is_none()
    }
}
/// Sample injected UTC with actual original controls before and after one bounded allocation.
pub(super) fn prepare(
    fact: FailedDesktopFact,
    original: IncidentOperationBudget,
    context: &JobContext,
    utc: impl FnOnce() -> std::result::Result<IncidentUtc, Box<dyn Error + Send + Sync>>,
) -> std::result::Result<FailureMaterial, FailureMaterialError> {
    let _original = context.memory().clone();
    let result = (|| -> std::result::Result<_, Box<dyn Error + Send + Sync>> {
        check(context, original)?;
        let _storage = context.memory().claim(size_of::<FailureMaterial>() as u64)?;
        let at = utc()?;
        check(context, original)?;
        Ok(FailureMaterial { fact, context: IncidentRequestContext::new(original, at), _storage })
    })();
    result.map_err(|cause| FailureMaterialError { cause, _original })
}
/// Reject actual cancellation/expiry or any effective deadline beyond the frozen interval.
fn check(
    context: &JobContext,
    original: IncidentOperationBudget,
) -> std::result::Result<(), Box<dyn Error + Send + Sync>> {
    if context.spec().reservation_bytes != 32 * 1024
        || context.cancellation_requested()
        || context.deadline().is_none_or(|until| until > original.until() || context.now() >= until)
        || context.now() < original.started()
    {
        return Err("Original failed desktop preparation controls rejected".into());
    }
    Ok(())
}
