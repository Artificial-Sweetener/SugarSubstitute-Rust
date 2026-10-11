// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve preparation causes while redacting environment names and values.
use std::{collections::TryReserveError, error::Error, fmt, io};
/// Reject an environment before native child creation or global mutation.
pub enum LaunchEnvironmentError {
    /// Snapshot/source input exceeds the UTF-16-unit limit.
    SourceTooLong,
    /// More than sixty-four overrides were supplied.
    TooManyOverrides,
    /// Snapshot or final variable count exceeds 1024.
    TooManyVariables,
    /// Snapshot lacks exact double-NUL termination or contains trailing data.
    InvalidSnapshot,
    /// A variable name is empty, contains NUL/equals or has invalid hidden shape.
    InvalidName,
    /// An override value contains NUL.
    InteriorNul,
    /// Snapshot has case-insensitively duplicate names.
    DuplicateSnapshot,
    /// Overrides have case-insensitively duplicate names, including removals.
    DuplicateOverride,
    /// An override attempts to insert project-private environment data.
    PrivateVariable,
    /// Final environment including its terminator exceeds 32767 units.
    BlockTooLong,
    /// An original fallible reservation failure.
    Allocation(TryReserveError),
    /// An original Windows ordinal comparison failure or invalid native result.
    Ordinal(io::Error),
}
impl fmt::Debug for LaunchEnvironmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl fmt::Display for LaunchEnvironmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SourceTooLong => "Environment input exceeds the source-size limit",
            Self::TooManyOverrides => "Environment exceeds the override-count limit",
            Self::TooManyVariables => "Environment exceeds the variable-count limit",
            Self::InvalidSnapshot => "Environment snapshot termination is invalid",
            Self::InvalidName => "Environment variable name is invalid",
            Self::InteriorNul => "Environment value contains an interior NUL",
            Self::DuplicateSnapshot => "Environment snapshot contains duplicate names",
            Self::DuplicateOverride => "Environment overrides contain duplicate names",
            Self::PrivateVariable => "Environment override inserts project-private data",
            Self::BlockTooLong => "Environment exceeds the native block-size limit",
            Self::Allocation(_) => "Environment storage allocation failed",
            Self::Ordinal(_) => "Windows environment name comparison failed",
        })
    }
}
impl Error for LaunchEnvironmentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Allocation(cause) => Some(cause),
            Self::Ordinal(cause) => Some(cause),
            _ => None,
        }
    }
}
