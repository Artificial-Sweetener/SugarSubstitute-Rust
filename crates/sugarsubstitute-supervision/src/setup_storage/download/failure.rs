// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve transfer, verification and durable-flush failures for the update coordinator.
use std::{error::Error, fmt, io};

/// Preparation failed without authorizing execution of cached bytes.
#[derive(Debug)]
pub enum DownloadFailure {
    /// Filesystem, transfer I/O, admission or cancellation cause.
    Storage(io::Error),
    /// Response framing disagrees with the authenticated request.
    Response(sugarsubstitute_update::delivery::Rejection),
    /// Complete bytes or metadata validity failed verification.
    Verification(sugarsubstitute_update::Error),
    /// Both a transfer and its final partial-file flush failed.
    Flush {
        /// Original transfer cause.
        transfer: Box<DownloadFailure>,
        /// Persistence cause; do not claim the partial was durably retained.
        flush: io::Error,
    },
}
impl fmt::Display for DownloadFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Storage(_) => "Update package I/O failed",
            Self::Response(_) => "Update package response rejected",
            Self::Verification(_) => "Update package verification failed",
            Self::Flush { .. } => "Update transfer and partial-file flush failed",
        })
    }
}
impl Error for DownloadFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(cause) => Some(cause),
            Self::Response(cause) => Some(cause),
            Self::Verification(cause) => Some(cause),
            Self::Flush { transfer, .. } => Some(transfer.as_ref()),
        }
    }
}
impl From<io::Error> for DownloadFailure {
    fn from(cause: io::Error) -> Self {
        Self::Storage(cause)
    }
}
impl From<sugarsubstitute_update::Error> for DownloadFailure {
    fn from(cause: sugarsubstitute_update::Error) -> Self {
        Self::Verification(cause)
    }
}
impl From<sugarsubstitute_update::delivery::Rejection> for DownloadFailure {
    fn from(cause: sugarsubstitute_update::delivery::Rejection) -> Self {
        Self::Response(cause)
    }
}
