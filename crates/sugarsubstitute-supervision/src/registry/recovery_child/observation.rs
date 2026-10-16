// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate exact original creation and independent UI terminals without inferring exits.
use super::*;
impl RecoveryChildCreation {
    /// Consume exact original terminals and return physical facts. Causes remain in this owner.
    /// # Errors
    /// Rejects wrong completion identity or preserves actual native/creation/cleanup failures.
    pub fn observe(&mut self) -> Result<RecoveryChildStatus, RegistryError> {
        if self.creation_completion.is_none() {
            self.creation_completion =
                self.creation.as_mut().and_then(OwnedJob::try_take_completion);
        }
        if self.cleanup_completion.is_none() {
            self.cleanup_completion = self.cleanup.as_mut().and_then(OwnedJob::try_take_completion);
        }
        if let Some(completion) = &self.creation_completion
            && self.creation.as_ref().is_none_or(|job| {
                job.scope_id() != completion.scope_id || job.spec() != completion.spec
            })
        {
            return Err(RegistryError::invalid("recovery_child_creation_terminal"));
        }
        if let Some(completion) = &self.cleanup_completion
            && self.cleanup.as_ref().is_none_or(|job| {
                job.scope_id() != completion.scope_id || job.spec() != completion.spec
            })
        {
            return Err(RegistryError::invalid("recovery_child_cleanup_terminal"));
        }
        let mut state = lock(&self.shared.state);
        if self.creation_completion.is_some()
            && let Some(permit) = state.permit.take()
        {
            state.watch = Some(permit.cancel_unbound());
        }
        if self.watch_completion.is_none() {
            self.watch_completion = state.watch.as_mut().and_then(OwnedJob::try_take_completion);
        }
        let mut exit = None;
        if let Some(completion) = &self.watch_completion {
            if (completion.scope_id, completion.spec) != self.watch_id {
                return Err(RegistryError::invalid("recovery_child_watch_terminal"));
            }
            if let JobOutcome::Success(actual) = &completion.outcome {
                validate_exit(state.created, self.origin.key().run().instance.get(), *actual)?;
                exit = Some(actual.exit_code);
                self.exit_verified = true;
            }
            if let JobOutcome::Failure(cause) = &completion.outcome {
                return Err(RegistryError::native_observation(
                    "recovery_child_watch",
                    cause.clone(),
                ));
            }
        }
        if let Some(completion) = &self.creation_completion
            && let JobOutcome::Failure(cause) = &completion.outcome
        {
            return Err(RegistryError::new("recovery_child_creation", cause.clone()));
        }
        if let Some(completion) = &self.cleanup_completion
            && let JobOutcome::Failure(cause) = &completion.outcome
        {
            return Err(cause.clone());
        }
        Ok(RecoveryChildStatus {
            created: state.created,
            resumed: state.resumed,
            exit,
            creation_settled: self.creation_completion.is_some() && state.native_retired,
            creation_terminal: self.creation_completion.as_ref().map(|value| value.outcome.kind()),
            watch_terminal: self.watch_completion.as_ref().map(|value| value.outcome.kind()),
        })
    }
    /// Prove that a capture owner can stop without abandoning a live or uncertain UI.
    /// Only the actual matching exit or an original checked no-process-output result qualifies.
    pub fn capture_retirement_ready(&self) -> bool {
        let state = lock(&self.shared.state);
        self.exit_verified
            || state.process_absent_proven
                && state.native_retired
                && (self.creation.is_none() || self.creation_completion.is_some())
    }
    /// Borrow the exact original creation completion and all retained original causes.
    pub fn creation_completion(
        &self,
    ) -> Option<&JobCompletion<ProcessCreationReport, ProcessCreationFailure>> {
        self.creation_completion.as_ref()
    }
    /// Borrow the exact independent observer completion, including cancellation and original causes.
    pub fn watch_completion(&self) -> Option<&JobCompletion<ProcessExit, NativeObservationError>> {
        self.watch_completion.as_ref()
    }
    /// Observe actual creation/output/watch retirement; a cancelled observer cannot prove UI exit.
    pub fn settled(&self) -> bool {
        let state = lock(&self.shared.state);
        (self.creation.is_none() || self.creation_completion.is_some())
            && state.native_retired
            && (!self.watch_admitted || self.watch_completion.is_some())
            && (self.cleanup.is_none() || self.cleanup_completion.is_some())
    }
}

/// Require the independent native watch's exact process birth and original correlation.
fn validate_exit(
    expected: Option<RecoveryProcessIdentity>,
    correlation: u64,
    actual: ProcessExit,
) -> Result<(), RegistryError> {
    if expected.is_none_or(|expected| {
        expected.pid() != actual.process_id || expected.birth() != actual.creation_time_100ns
    }) || actual.correlation.get() != correlation
    {
        return Err(RegistryError::invalid("recovery_child_exit_identity"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ui_exit_requires_independent_actual_birth_and_original_watch_correlation_including259()
    -> Result<(), Box<dyn std::error::Error>> {
        let expected = RecoveryProcessIdentity::new(7, 11)?;
        let actual = ProcessExit {
            correlation: sugarsubstitute_execution::windows::NativeCorrelation::new(3)?,
            process_id: 7,
            creation_time_100ns: 11,
            exit_code: 259,
        };
        validate_exit(Some(expected), 3, actual)?;
        assert_eq!(actual.exit_code, 259);
        assert!(validate_exit(None, 3, actual).is_err());
        assert!(validate_exit(Some(expected), 4, actual).is_err());
        assert!(validate_exit(Some(RecoveryProcessIdentity::new(8, 11)?), 3, actual).is_err());
        assert!(validate_exit(Some(RecoveryProcessIdentity::new(7, 12)?), 3, actual).is_err());
        Ok(())
    }
}
