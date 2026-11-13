// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve verification and publication failures without returning uncommitted authority.
use super::protection::DomainFailure;
use std::{error::Error, fmt, io};
use sugarsubstitute_update::Error as Verification;

/// Update trust did not complete its durable transaction.
#[derive(Debug)]
pub enum TrustFailure {
    /// Admission, custody, record or native I/O failed before authority could be returned.
    Storage(io::Error),
    /// A typed operation failure and native retirement failure are both retained.
    Retirement {
        /// Earlier verification or publication failure.
        operation: Box<TrustFailure>,
        /// Native protection retirement failure.
        retirement: io::Error,
    },
    /// Another holder kept the trust writer past the bounded wait or the job deadline.
    LockTimeout,
    /// The job was cancelled while waiting for the trust writer.
    LockCancelled,
    /// Verification failed; any previously accepted progress was saved successfully.
    Verification(Verification),
    /// Accepted trust could not be saved. No offer is returned; both failures are retained.
    Publication {
        /// Publication or serialization cause.
        storage: io::Error,
        /// A verification failure that preceded publication, when present.
        verification: Option<Verification>,
    },
}
impl fmt::Display for TrustFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Retirement { .. } => "Update trust and protection retirement failed",
            Self::Storage(_) => "Update trust storage failed",
            Self::LockTimeout => "Update trust stayed locked by another process",
            Self::LockCancelled => "Update trust wait was cancelled",
            Self::Verification(_) => "Update verification failed",
            Self::Publication { .. } => "Update trust could not be committed",
        })
    }
}
impl Error for TrustFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(cause) | Self::Publication { storage: cause, .. } => Some(cause),
            Self::Verification(cause) => Some(cause),
            Self::Retirement { operation, .. } => Some(operation.as_ref()),
            Self::LockTimeout | Self::LockCancelled => None,
        }
    }
}
impl From<io::Error> for TrustFailure {
    fn from(cause: io::Error) -> Self {
        Self::Storage(cause)
    }
}
impl From<super::super::short_lock::Refused> for TrustFailure {
    fn from(refused: super::super::short_lock::Refused) -> Self {
        use super::super::short_lock::Refused;
        match refused {
            Refused::Timeout => Self::LockTimeout,
            Refused::Cancelled => Self::LockCancelled,
            Refused::Storage(cause) => Self::Storage(cause),
        }
    }
}
impl From<Verification> for TrustFailure {
    fn from(cause: Verification) -> Self {
        Self::Verification(cause)
    }
}

impl From<DomainFailure<Self>> for TrustFailure {
    fn from(failure: DomainFailure<Self>) -> Self {
        match failure {
            DomainFailure::Operation(cause) => cause,
            DomainFailure::Native { cause, operation: None } => Self::Storage(cause),
            DomainFailure::Native { cause, operation: Some(operation) } => {
                Self::Retirement { operation: Box::new(operation), retirement: cause }
            }
        }
    }
}
