// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Distinguish native, protocol, panic, cancellation and timeout terminal failures.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::{JobId, JobOutcome, PanicCause};
use sugarsubstitute_execution::{windows::NativeCorrelation, windows_fixture::FixtureChildReport};

#[derive(Debug)]
pub(super) enum ChildFailure<E> {
    Native(E),
    Protocol,
    Panicked(ChildPanic),
    Cancelled,
    TimedOut,
    Coalesced(JobId),
}
impl<E: Error + 'static> fmt::Display for ChildFailure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Self::Coalesced(replacement) = self {
            return write!(f, "Held child unexpectedly coalesced by job {}", replacement.get());
        }
        f.write_str(match self {
            Self::Native(_) => "Held child native operation failed",
            Self::Protocol => "Held child terminal evidence is incomplete or foreign",
            Self::Panicked(_) => "Held child authoritative native owner panicked",
            Self::Cancelled => "Held child was actually cancelled before successful Release",
            Self::TimedOut => "Held child actually timed out at its original admitted deadline",
            Self::Coalesced(_) => "Held child was unexpectedly coalesced",
        })
    }
}
impl<E: Error + 'static> Error for ChildFailure<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Native(error) => Some(error),
            Self::Panicked(cause) => Some(cause),
            _ => None,
        }
    }
}
pub(super) struct ChildPanic(pub(super) PanicCause);
impl fmt::Debug for ChildPanic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChildPanic").field("retained_utf8_bytes", &self.0.as_str().len()).finish()
    }
}
impl fmt::Display for ChildPanic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Supervised held-child owner panic")
    }
}
impl Error for ChildPanic {}

pub(super) fn terminal<E>(
    correlation: NativeCorrelation,
    outcome: JobOutcome<FixtureChildReport, E>,
) -> std::result::Result<FixtureChildReport, ChildFailure<E>> {
    match outcome {
        JobOutcome::Success(report)
            if report.correlation == correlation
                && report.inheritance_cleared
                && report.ready_signalled
                && report.release_observed =>
        {
            Ok(report)
        }
        JobOutcome::Success(_) => Err(ChildFailure::Protocol),
        JobOutcome::Failure(error) => Err(ChildFailure::Native(error)),
        JobOutcome::Panicked(cause) => Err(ChildFailure::Panicked(ChildPanic(cause))),
        JobOutcome::Cancelled => Err(ChildFailure::Cancelled),
        JobOutcome::TimedOut => Err(ChildFailure::TimedOut),
        JobOutcome::Coalesced { replacement } => Err(ChildFailure::Coalesced(replacement)),
    }
}
