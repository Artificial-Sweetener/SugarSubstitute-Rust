// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve typed unexpected native terminal classifications without secret diagnostics.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::{JobId, JobOutcome, PanicCause};
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    LifecyclePipeFailure, LifecyclePipeReport,
};

#[derive(Debug)]
pub(super) enum TerminalFailure {
    Native(LifecyclePipeFailure),
    Panicked(OwnedPanic),
    Cancelled,
    TimedOut,
    Coalesced(JobId),
    UnexpectedSuccess,
}
impl fmt::Display for TerminalFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Native(_) => "Lifecycle transport failed unexpectedly",
            Self::Panicked(_) => "Lifecycle transport owner panicked",
            Self::Cancelled => "Lifecycle transport cancelled before required evidence",
            Self::TimedOut => "Lifecycle transport reached its original deadline",
            Self::Coalesced(_) => "Lifecycle transport was unexpectedly coalesced",
            Self::UnexpectedSuccess => "Lifecycle transport success lacks intentional retirement",
        })?;
        if let Self::Coalesced(id) = self {
            write!(f, " by job {}", id.get())?;
        }
        Ok(())
    }
}
impl Error for TerminalFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Native(error) => Some(error),
            Self::Panicked(error) => Some(error),
            _ => None,
        }
    }
}
pub(super) struct OwnedPanic(PanicCause);
impl fmt::Debug for OwnedPanic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedPanic").field("retained_utf8_bytes", &self.0.as_str().len()).finish()
    }
}
impl fmt::Display for OwnedPanic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Retained lifecycle owner panic")
    }
}
impl Error for OwnedPanic {}
pub(super) fn unexpected(
    outcome: JobOutcome<LifecyclePipeReport, LifecyclePipeFailure>,
) -> TerminalFailure {
    match outcome {
        JobOutcome::Success(_) => TerminalFailure::UnexpectedSuccess,
        JobOutcome::Failure(error) => {
            tracing::error!(operation="lifecycle_fixture_terminal", stage=error.native_cause().stage(),
                win32_code=error.win32_code(), cause=?Error::source(error.native_cause()),
                owner_panic=error.contains_owner_panic(), requires_process_shutdown=error.requires_process_shutdown(),
                "Retained unexpected native lifecycle transport failure");
            TerminalFailure::Native(error)
        }
        JobOutcome::Panicked(cause) => {
            tracing::error!(
                operation = "lifecycle_fixture_terminal",
                retained_utf8_bytes = cause.as_str().len(),
                "Retained lifecycle transport owner panic"
            );
            TerminalFailure::Panicked(OwnedPanic(cause))
        }
        JobOutcome::Cancelled => TerminalFailure::Cancelled,
        JobOutcome::TimedOut => TerminalFailure::TimedOut,
        JobOutcome::Coalesced { replacement } => TerminalFailure::Coalesced(replacement),
    }
}
/// A negative role accepts only the concrete peer-disconnect native observation.
pub(super) fn expected_disconnect(error: &LifecyclePipeFailure) -> bool {
    disconnect_facts(
        error.native_cause().stage(),
        error.win32_code(),
        error.requires_process_shutdown(),
        error.contains_owner_panic(),
    )
}
pub(super) fn disconnect_facts(
    stage: &str,
    code: Option<u32>,
    unrestored: bool,
    panicked: bool,
) -> bool {
    stage == "pipe_completion" && matches!(code, Some(109 | 233)) && !unrestored && !panicked
}
