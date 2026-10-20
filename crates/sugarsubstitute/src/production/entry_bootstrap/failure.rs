// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Close entry admission on actual native terminal observations and preserve their original causes.
use super::*;

impl EntryBootstrap {
    /// Preserve native terminal observations before another preparation can be admitted.
    pub(super) fn observe(
        &mut self,
        observation: Option<EntryObservation>,
    ) -> Result<Option<EntryObservation>, EntryBootstrapIssue> {
        if let Some(observation) = &observation {
            tracing::info!(
                operation = "entry_bootstrap_progress",
                ?observation,
                "Coordinator discovery advanced"
            );
        }
        let failure = match observation {
            Some(EntryObservation::Failed(cause)) => {
                self.retain_native_failure(cause);
                return Err(EntryBootstrapIssue::NativeOwner);
            }
            Some(EntryObservation::Degraded(reason)) => {
                Failure::Submission(Box::new(EntryBootstrapIssue::Degraded(reason)))
            }
            Some(EntryObservation::Stopped(kind)) if !self.closing => {
                Failure::Submission(Box::new(EntryBootstrapIssue::Stopped(kind)))
            }
            other => return Ok(other),
        };
        if self.failure.is_none() {
            self.failure = Some(failure);
        }
        self.request_close();
        Err(EntryBootstrapIssue::NativeOwner)
    }

    /// Keep native startup and cleanup causes in the same slots for returned and queued failures.
    pub(super) fn retain_native_failure(&mut self, cause: EntryError) {
        if self.failure.is_none() && self.failed_preparation.is_none() {
            self.failure = Some(Failure::Native(cause));
        } else if self.cleanup_failure.is_none() {
            self.cleanup_failure = Some(CleanupFailure::Native(cause));
        }
        self.request_close();
    }

    /// Borrow a retained primary source; complete preparation outcomes stay owned here.
    pub(crate) fn failure_source(&self) -> Option<&(dyn Error + 'static)> {
        if let Some(completion) = &self.failed_preparation {
            return match &completion.outcome {
                JobOutcome::Failure(cause) => Some(cause),
                _ => None,
            };
        }
        match self.failure.as_ref()? {
            Failure::Submission(cause) => Some(cause.as_ref()),
            Failure::Adoption(rejected) => Some(rejected.cause()),
            Failure::Input(cause) => Some(cause),
            Failure::Admission(cause) => Some(cause),
            Failure::Native(cause) => Some(cause),
        }
    }
    /// Borrow the first cleanup failure separately from original startup failure.
    pub(crate) fn cleanup_failure(&self) -> Option<&(dyn Error + 'static)> {
        match self.cleanup_failure.as_ref()? {
            CleanupFailure::Admission(cause) => Some(cause),
            CleanupFailure::Native(cause) => Some(cause),
        }
    }
    /// Borrow the original factory's later control observation without separating custody.
    pub(crate) fn post_control(&self) -> Option<&sugarsubstitute_execution::EntryPreparationError> {
        let JobOutcome::Failure(failure) = &self.failed_preparation.as_ref()?.outcome else {
            return None;
        };
        let sugarsubstitute_execution::EntryPreparationCause::Factory(cause) = failure.cause()
        else {
            return None;
        };
        cause.post_control()
    }
}
