// SPDX-License-Identifier: AGPL-3.0-or-later
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
/// States which repository operation failed. The cause is the error's source, so a report that
/// lists every cause says each one once.
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Http(_) => "Update repository transport failed",
            Self::Trust(_) => "Update repository trust failed",
            Self::Verification(_) => "Update preparation rejected",
            Self::Download(_) => "Update repository download failed",
            Self::Io(_) => "Update repository I/O failed",
        })
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

#[cfg(test)]
mod tests {
    use super::Failure;
    use sugarsubstitute_contracts::update::attempt::{AttemptPhase, UpdateReport};

    /// The update report lists the failed operation, then its cause, each exactly once.
    #[test]
    fn a_report_states_the_failed_operation_and_its_cause_once_each() {
        let failure = Failure::Io(std::io::Error::other("The disk is gone"));
        assert_eq!(failure.to_string(), "Update repository I/O failed");
        let report = UpdateReport::from_error(None, AttemptPhase::Checking, &failure);
        assert_eq!(report.causes, ["Update repository I/O failed", "The disk is gone"]);
    }
}
