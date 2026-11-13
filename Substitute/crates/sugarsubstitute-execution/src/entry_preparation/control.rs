// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve original preparation control categories without formatting panic data.

use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::{ExecutionError, PanicCause, TimeError};

/// Preserve an observation from the original admitted preparation's control ports.
/// Cancellation and expiry describe admission decisions, not native termination.
pub enum EntryPreparationControlCause {
    /// Actual original cancellation was observed.
    Cancelled,
    /// The unchanged original effective cutoff was reached.
    Deadline,
    /// Preserve actual monotonic clock failure or regression.
    Clock(TimeError),
    /// Preserve an actual fatal execution clock/provider failure unchanged.
    Unexpected(ExecutionError),
    /// Preserve the bounded actual panic without printing its payload.
    ProviderPanic(PanicCause),
    /// Preserve an original ownership/control rejection without reducing its cause.
    Boundary(Box<super::EntryPreparationInputError>),
}
impl EntryPreparationControlCause {
    /// Borrow the actual bounded panic for explicit fatal handling.
    /// The payload is diagnostic data and is not automatically logged.
    pub const fn panic_cause(&self) -> Option<&PanicCause> {
        match self {
            Self::ProviderPanic(cause) => Some(cause),
            _ => None,
        }
    }
    /// Identify the observed category without exposing diagnostic payloads.
    pub const fn category(&self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::Deadline => "deadline",
            Self::Clock(_) => "clock",
            Self::Unexpected(_) => "unexpected",
            Self::ProviderPanic(_) => "provider_panic",
            Self::Boundary(_) => "boundary",
        }
    }
}
impl fmt::Debug for EntryPreparationControlCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntryPreparationControlCause")
            .field("category", &self.category())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for EntryPreparationControlCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Original entry preparation control failed ({})", self.category())
    }
}
impl Error for EntryPreparationControlCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Clock(cause) => Some(cause),
            Self::Unexpected(cause) => Some(cause),
            Self::Boundary(cause) => Some(cause.as_ref()),
            _ => None,
        }
    }
}
