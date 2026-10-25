// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve original selected-input failures and their independent control observations.
use std::{collections::TryReserveError, error::Error, fmt, io};
use sugarsubstitute_execution::EntryPreparationError;

/// Selected-file failure retaining genuine original preparation custody in control causes.
pub enum PreparationFileError {
    /// Original cancellation, publication closure or cutoff rejected further input work.
    Control(EntryPreparationError),
    /// Actual filesystem failure, retained before mandatory post-effect observation.
    Read {
        /// Original operating-system I/O failure.
        source: io::Error,
        /// Independent later control failure, with unchanged original custody.
        after: Option<EntryPreparationError>,
    },
    /// The original finite input allocation failed.
    Capacity(TryReserveError),
    /// Input or sentinel allocation exceeds the supplied or original reservation bound.
    TooLarge,
}
impl fmt::Debug for PreparationFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Control(_) => formatter.write_str("PreparationFileError::Control"),
            Self::Read { after, .. } => formatter
                .debug_struct("PreparationFileError::Read")
                .field("post_control", after)
                .finish_non_exhaustive(),
            Self::Capacity(_) => formatter.write_str("PreparationFileError::Capacity"),
            Self::TooLarge => formatter.write_str("PreparationFileError::TooLarge"),
        }
    }
}
impl fmt::Display for PreparationFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Selected preparation input could not be read")
    }
}
impl Error for PreparationFileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Control(cause) => Some(cause),
            Self::Read { source, .. } => Some(source),
            Self::Capacity(cause) => Some(cause),
            Self::TooLarge => None,
        }
    }
}
