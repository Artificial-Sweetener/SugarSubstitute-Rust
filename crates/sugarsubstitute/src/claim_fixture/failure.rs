// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve typed failures without rendering a worker's private panic payload.
use super::Result;
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::JobOutcome;
struct PanicFailure {
    cause: std::io::Error,
}
impl fmt::Debug for PanicFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PanicFailure").finish_non_exhaustive()
    }
}
impl fmt::Display for PanicFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Claim fixture worker panicked")
    }
}
impl Error for PanicFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}
/// Convert an actually transferred terminal while retaining native and panic causes.
pub(super) fn terminal<T, E: Error + 'static>(outcome: JobOutcome<T, E>) -> Result<T> {
    match outcome {
        JobOutcome::Success(value) => Ok(value),
        JobOutcome::Failure(cause) => Err(Box::new(cause)),
        JobOutcome::Panicked(cause) => {
            Err(Box::new(PanicFailure { cause: std::io::Error::other(cause.as_str().to_owned()) }))
        }
        JobOutcome::Cancelled => Err("Claim fixture job was actually cancelled".into()),
        JobOutcome::TimedOut => Err("Claim fixture job exceeded its original deadline".into()),
        JobOutcome::Coalesced { .. } => Err("Claim fixture job was unexpectedly coalesced".into()),
    }
}

/// Hold every failure from one bounded turn without rendering private causes.
/// Parent drains at most14 fallible facts, child at most4; sixteen slots cover both.
pub(super) struct TurnFailures {
    causes: [Option<Box<dyn Error>>; 16],
    count: usize,
    omitted: usize,
}
impl TurnFailures {
    pub(super) fn new() -> Self {
        Self { causes: std::array::from_fn(|_| None), count: 0, omitted: 0 }
    }
    pub(super) fn record(&mut self, operation: &'static str, cause: Box<dyn Error>) {
        if self.count != 0 {
            tracing::error!(
                operation,
                retained_secondary_causes = self.count,
                "Claim qualification retained an additional original cause; private payload remains redacted"
            );
        }
        if let Some(slot) = self.causes.get_mut(self.count) {
            *slot = Some(cause);
            self.count += 1;
        } else {
            self.omitted += 1;
            tracing::error!(
                operation,
                omitted_causes = self.omitted,
                "Claim qualification failure inventory exceeded its bounded turn"
            );
        }
    }
    pub(super) fn finish(mut self) -> Result<()> {
        match self.count {
            0 => Ok(()),
            1 => Err(self.causes[0].take().ok_or("Claim failure slot missing")?),
            _ => Err(Box::new(self)),
        }
    }
}
impl fmt::Debug for TurnFailures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TurnFailures")
            .field("retained_causes", &self.count)
            .field("omitted_causes", &self.omitted)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for TurnFailures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Claim qualification turn failed with {} retained causes", self.count)
    }
}
impl Error for TurnFailures {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.causes[0].as_deref()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two_native_failures_preserve_first_and_secondary_without_private_display() -> Result<()> {
        let mut failures = TurnFailures::new();
        failures.record("first_native", Box::new(std::io::Error::other("first-private-path")));
        failures.record("cleanup_native", Box::new(std::io::Error::other("second-private-token")));
        assert_eq!(failures.count, 2);
        assert_eq!(
            failures.source().ok_or("First source missing")?.to_string(),
            "first-private-path"
        );
        assert_eq!(
            failures.causes[1].as_ref().ok_or("Second source missing")?.to_string(),
            "second-private-token"
        );
        assert!(!format!("{failures:?} {failures}").contains("private"));
        let error = failures.finish().err().ok_or("Missing combined failure")?;
        assert_eq!(
            error.source().ok_or("Combined source missing")?.to_string(),
            "first-private-path"
        );
        Ok(())
    }
}
