// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Preserve actionable repository and process failures across verification boundaries.

use std::{error::Error, fmt, path::Path};

/// Keep policy violations distinct from external failures.
#[derive(Debug)]
pub(crate) enum Failure {
    /// Preserve the operation and original external error.
    External { context: String, source: Box<dyn Error> },
    /// Report an invariant that the maintainer must resolve.
    Policy(String),
    /// Preserve a failed child command's exit status.
    Process { command: String, code: Option<i32> },
}

/// Carry typed failures throughout repository tooling.
pub(crate) type Result<T> = std::result::Result<T, Failure>;

impl Failure {
    /// Attach operation context without replacing the original cause.
    pub(crate) fn external(context: impl Into<String>, source: impl Error + 'static) -> Self {
        Self::External { context: context.into(), source: Box::new(source) }
    }
}

impl fmt::Display for Failure {
    /// Render one actionable diagnostic for command-line callers.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::External { context, source } => write!(formatter, "{context}: {source}"),
            Self::Policy(message) => formatter.write_str(message),
            Self::Process { command, code } => write!(formatter, "{command} failed: exit {code:?}"),
        }
    }
}

impl Error for Failure {
    /// Expose external causes for diagnostic consumers.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::External { source, .. } => Some(source.as_ref()),
            Self::Policy(_) | Self::Process { .. } => None,
        }
    }
}

/// Read UTF-8 source with a newline-stable cross-platform representation.
pub(crate) fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path)
        .map(|text| text.replace("\r\n", "\n").replace('\r', "\n"))
        .map_err(|error| Failure::external(format!("read {}", path.display()), error))
}

/// Reject a violated policy with its owner-specific corrective context.
pub(crate) fn require(condition: bool, message: impl Into<String>) -> Result<()> {
    if condition { Ok(()) } else { Err(Failure::Policy(message.into())) }
}
