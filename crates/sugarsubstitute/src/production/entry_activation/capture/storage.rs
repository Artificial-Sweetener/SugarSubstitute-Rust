// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain independently transferred protected-root custody through checked final Root closure.
use super::*;
use sugarsubstitute_contracts::execution::{ExecutionPort, JobCompletion, JobOutcome};
use sugarsubstitute_crash::{CaptureRootSource, CrashError};
/// The original source, terminal and causes retain every actual claim until final Root disposal.
pub(super) struct CaptureRootStorage {
    source: Option<CaptureRootSource>,
    close: Option<OwnedJob<(), CrashError>>,
    completion: Option<JobCompletion<(), CrashError>>,
    failure: Option<CrashError>,
    retired: bool,
}
impl CaptureRootStorage {
    pub(super) const fn new() -> Self {
        Self { source: None, close: None, completion: None, failure: None, retired: false }
    }
    pub(super) fn retain(&mut self, source: CaptureRootSource) {
        self.source = Some(source);
    }
    pub(super) fn source(&self) -> Option<&CaptureRootSource> {
        self.source.as_ref()
    }
    /// Adopt the exact actual completion before returning any retained failure.
    pub(super) fn observe(&mut self) -> Result<()> {
        if let Some(completion) = self.close.as_mut().and_then(OwnedJob::try_take_completion) {
            self.close = None;
            match &completion.outcome {
                JobOutcome::Success(()) => self.retired = true,
                JobOutcome::Failure(cause) => self.failure = Some(cause.clone()),
                _ => {}
            }
            self.completion = Some(completion);
        }
        if self.completion.as_ref().is_some_and(|done| {
            !matches!(done.outcome, JobOutcome::Success(()) | JobOutcome::Failure(_))
        }) {
            return Err("Actual protected Root retirement ended without successful checked closure; original completion retained".into());
        }
        if let Some(cause) = &self.failure {
            return Err(Box::new(cause.clone()));
        }
        Ok(())
    }
    /// Use the original Root cleanup interval after every granted reader has retired.
    pub(super) fn close(
        &mut self,
        budget: ShutdownBudget,
        resources: &EntryResources<'_>,
    ) -> Result<bool> {
        if self.source.is_some()
            && !self.retired
            && self.close.is_none()
            && self.failure.is_none()
            && self.completion.is_none()
        {
            let source = self.source.as_ref().ok_or("Actual Root storage source missing")?.clone();
            self.close = Some(resources.client.submit_blocking(
                resources.cleanup,
                JobSpec {
                    id: resources.jobs.allocate()?,
                    operation: Operation::NativeLifecycle,
                    class: JobClass::InfrastructureLifecycle,
                    deadline: Some(budget.settle_until),
                    reservation_bytes: 4096,
                    replacement: None,
                },
                move |context| source.retire(&context),
            )?);
        }
        self.observe()?;
        Ok(self.source.is_none() || self.retired)
    }
}
