// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the owner and cause of discovery/preparation failures for caller retry policy.
use crate::{
    setup_storage::{DownloadFailure, TrustFailure},
    update_http,
};

/// A failed repository operation is never an up-to-date result.
#[derive(Debug)]
pub enum Failure {
    /// Origin, HTTP, transport or retirement failure; custody remains with the repository owner.
    Http(update_http::Failure),
    /// Protected trust verification/publication failure.
    Trust(TrustFailure),
    /// Authenticated offer does not permit preparation.
    Verification(sugarsubstitute_update::Error),
    /// Owned download framing, persistence or content verification failure.
    Download(DownloadFailure),
    /// Admission or native I/O failure.
    Io(std::io::Error),
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(cause) => write!(f, "Update repository transport failed: {cause}"),
            Self::Trust(cause) => write!(f, "Update repository trust failed: {cause}"),
            Self::Verification(cause) => write!(f, "Update preparation rejected: {cause}"),
            Self::Download(cause) => write!(f, "Update repository download failed: {cause}"),
            Self::Io(cause) => write!(f, "Update repository I/O failed: {cause}"),
        }
    }
}
impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Http(cause) => cause,
            Self::Trust(cause) => cause,
            Self::Verification(cause) => cause,
            Self::Download(cause) => cause,
            Self::Io(cause) => cause,
        })
    }
}
impl From<update_http::Failure> for Failure {
    fn from(cause: update_http::Failure) -> Self {
        Self::Http(cause)
    }
}
impl From<TrustFailure> for Failure {
    fn from(cause: TrustFailure) -> Self {
        Self::Trust(cause)
    }
}
impl From<sugarsubstitute_update::Error> for Failure {
    fn from(cause: sugarsubstitute_update::Error) -> Self {
        Self::Verification(cause)
    }
}
impl From<DownloadFailure> for Failure {
    fn from(cause: DownloadFailure) -> Self {
        Self::Download(cause)
    }
}
impl From<std::io::Error> for Failure {
    fn from(cause: std::io::Error) -> Self {
        Self::Io(cause)
    }
}
