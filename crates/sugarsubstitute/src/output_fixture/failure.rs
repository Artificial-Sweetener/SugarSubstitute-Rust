// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve bounded panic causes while keeping outer fixture diagnostics redacted.
use super::Result;
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::{JobOutcome, PanicCause};
struct PanicFailure {
    cause: std::io::Error,
}
impl fmt::Debug for PanicFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OutputPanicFailure { redacted }")
    }
}
impl fmt::Display for PanicFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Authoritative output fixture worker panicked")
    }
}
impl Error for PanicFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}
pub(super) fn panic(cause: PanicCause) -> Box<dyn Error> {
    Box::new(PanicFailure { cause: std::io::Error::other(cause.as_str().to_owned()) })
}
pub(super) fn outcome<T, E: Error + 'static>(outcome: JobOutcome<T, E>) -> Result<T> {
    match outcome {
        JobOutcome::Success(value) => Ok(value),
        JobOutcome::Failure(cause) => Err(Box::new(cause)),
        JobOutcome::Panicked(cause) => Err(panic(cause)),
        _ => Err("Output fixture work stopped without actual successful completion".into()),
    }
}
