// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve genuine original binding rejections without formatting provider data.

use super::control::EntryPreparationControlCause;
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::{ExecutionError, MemoryClaimError};

/// Reject an original preparation association or control observation.
/// Source-backed errors are adopted with the original whole owner before escaping
/// an input borrow; this nonclone cause alone never proves native authority.
pub enum EntryPreparationInputError {
    /// The actual original owner differs from the receiving execution adapter.
    ForeignOwner,
    /// The genuine record or unchanged original setup association differs.
    OriginalSetupMismatch,
    /// The original owner or its publication permission no longer accepts work.
    Closed,
    /// Preserve original cancellation, cutoff, clock or provider observation.
    Control(EntryPreparationControlCause),
    /// Preserve the original fatal execution adapter cause without coercion.
    Unexpected(ExecutionError),
    /// Preserve the unallocated original full-claim admission rejection.
    Claim(MemoryClaimError),
}
impl EntryPreparationInputError {
    /// Identify the rejection without exposing paths, provider text or panic data.
    pub const fn category(&self) -> &'static str {
        match self {
            Self::ForeignOwner => "foreign_owner",
            Self::OriginalSetupMismatch => "original_setup_mismatch",
            Self::Closed => "closed",
            Self::Control(_) => "control",
            Self::Unexpected(_) => "unexpected",
            Self::Claim(_) => "claim",
        }
    }
}
impl fmt::Debug for EntryPreparationInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EntryPreparationInputError")
            .field("category", &self.category())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for EntryPreparationInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Original entry preparation rejected ({})", self.category())
    }
}
impl Error for EntryPreparationInputError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Control(cause) => Some(cause),
            Self::Unexpected(cause) => Some(cause),
            Self::Claim(cause) => Some(cause),
            _ => None,
        }
    }
}
