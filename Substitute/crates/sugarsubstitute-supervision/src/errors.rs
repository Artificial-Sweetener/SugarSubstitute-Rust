// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve allocation causes while keeping launch paths and arguments out of diagnostics.
use std::{collections::TryReserveError, error::Error, fmt};

/// Reject launch preparation before any native process operation.
pub enum LaunchCommandError {
    /// More than sixty-four separately supplied arguments were provided.
    TooManyArguments,
    /// A path or aggregate unquoted argv exceeds the bounded UTF-16 source size.
    SourceTooLong,
    /// An executable, directory or argument contains a reserved NUL code unit.
    InteriorNul,
    /// The executable is not a supported fully-qualified lexical file path.
    InvalidExecutable,
    /// The working directory is not a supported fully-qualified lexical directory path.
    InvalidWorkingDirectory,
    /// Quoting expansion exceeds 32767 UTF-16 units including the final NUL.
    CommandLineTooLong,
    /// Native-input storage could not be reserved; the original allocation cause is retained.
    Allocation(TryReserveError),
}
impl fmt::Debug for LaunchCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl fmt::Display for LaunchCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooManyArguments => "Windows launch exceeds the argument-count limit",
            Self::SourceTooLong => "Windows launch exceeds the source-size limit",
            Self::InteriorNul => "Windows launch contains an interior NUL",
            Self::InvalidExecutable => "Windows launch requires a fully-qualified executable path",
            Self::InvalidWorkingDirectory => {
                "Windows launch requires a fully-qualified directory path"
            }
            Self::CommandLineTooLong => "Windows launch exceeds the command-line limit",
            Self::Allocation(_) => "Windows launch storage allocation failed",
        })
    }
}
impl Error for LaunchCommandError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Allocation(cause) => Some(cause),
            _ => None,
        }
    }
}
