// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve receiving-file control and native causes without exposing selected paths.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::AdmissionError;

/// Closed receiving-file rejection categories, independent of report contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportFileCode {
    /// Retained object, phase or cleanup origin does not match.
    Identity,
    /// Selected path or object violates the bounded local-file contract.
    Destination,
    /// An existing destination requires explicit overwrite consent.
    Consent,
    /// Write size or accepted native byte count violates the bounded stream contract.
    Bounds,
}

/// Original receiving-file failure retained by its admitted feature operation.
#[derive(Debug)]
pub enum ReportFileError {
    /// Validation rejected inert selection data or current native observations.
    Rejected(ReportFileCode),
    /// Actual original execution cancellation, deadline or admission failure.
    Control(AdmissionError),
    /// Actual synchronous operating-system failure, preserved without formatting paths.
    Native(windows::core::Error),
}
impl fmt::Display for ReportFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(code) => write!(formatter, "Receiving file rejected: {code:?}"),
            Self::Control(_) => formatter.write_str("Receiving file execution control failed"),
            Self::Native(_) => formatter.write_str("Receiving file native operation failed"),
        }
    }
}
impl Error for ReportFileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Control(cause) => Some(cause),
            Self::Native(cause) => Some(cause),
            Self::Rejected(_) => None,
        }
    }
}
