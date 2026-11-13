// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain operation and retirement causes independently of native handle ownership.
use std::{error::Error, fmt, io};
/// Update transport failed; native custody remains with its original `Http` owner.
#[derive(Debug)]
pub enum Failure {
    /// Native I/O, admission, cancellation or size limit.
    Io(io::Error),
    /// Origin or response-header policy failed.
    Policy(sugarsubstitute_update::http::Rejection),
    /// Resumable storage or package response validation failed.
    Download(crate::setup_storage::DownloadFailure),
    /// An operation and native retirement both failed; the original owner must be retained.
    Retirement {
        /// Original request/storage cause.
        operation: Box<Failure>,
        /// Native closure cause.
        retirement: io::Error,
    },
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Io(_) => "Update HTTP operation failed",
            Self::Policy(_) => "Update HTTP policy rejected input",
            Self::Download(_) => "Update package transfer failed",
            Self::Retirement { .. } => "Update HTTP operation and retirement failed",
        })
    }
}
impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(cause) => Some(cause),
            Self::Policy(cause) => Some(cause),
            Self::Download(cause) => Some(cause),
            Self::Retirement { operation, .. } => Some(operation.as_ref()),
        }
    }
}
impl From<io::Error> for Failure {
    fn from(cause: io::Error) -> Self {
        Self::Io(cause)
    }
}
impl From<sugarsubstitute_update::http::Rejection> for Failure {
    fn from(cause: sugarsubstitute_update::http::Rejection) -> Self {
        Self::Policy(cause)
    }
}
impl From<crate::setup_storage::DownloadFailure> for Failure {
    fn from(cause: crate::setup_storage::DownloadFailure) -> Self {
        Self::Download(cause)
    }
}
