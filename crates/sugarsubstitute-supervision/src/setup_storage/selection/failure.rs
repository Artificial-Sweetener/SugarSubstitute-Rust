// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve selection/domain failures alongside native protection retirement.
use super::protection::DomainFailure;
use std::{error::Error, fmt, io};
use sugarsubstitute_contracts::installation::Rejection;

/// Preserve storage and domain causes while distinguishing stale writers from corruption.
#[derive(Debug)]
pub enum SelectionFailure {
    /// Another holder kept the selection lock past the bounded wait or the job deadline.
    LockTimeout,
    /// The job was cancelled while waiting for the selection lock.
    LockCancelled,
    /// The accepted generation changed after this launch was reserved. The reserved executable
    /// is no longer the selected application; the launcher must reserve again and start the
    /// newly accepted generation instead.
    Superseded,
    /// Another native owner holds installation-run exclusion; this grants no trial authority.
    RunBusy(io::Error),
    /// Native storage, cancellation, or exclusion failure.
    Storage(io::Error),
    /// Malformed or unsupported state; the original bytes remain intact.
    Record(Rejection),
    /// Another writer published a different selection; reload before making a new decision.
    Changed,
    /// No initialized selection is available at the requested installation.
    Missing,
    /// Replacement does not advance this installation's exact revision by one.
    Revision,
    /// Activation changes require the live installation run owner.
    RunOwnership,
    /// A domain failure and native protection retirement both failed.
    Retirement {
        /// Original domain failure.
        operation: Box<SelectionFailure>,
        /// Native retirement failure.
        retirement: io::Error,
    },
}
impl fmt::Display for SelectionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LockTimeout => "Installation selection stayed locked by another process",
            Self::LockCancelled => "Installation selection wait was cancelled",
            Self::Superseded => "Installation selection accepted a different application",
            Self::RunBusy(_) => "Another application invocation owns this installation",
            Self::Storage(_) => "Installation selection storage failed",
            Self::Record(_) => "Installation selection record rejected",
            Self::Changed => "Installation selection changed before publication",
            Self::Missing => "Installation selection is missing",
            Self::Revision => "Installation selection revision is not contiguous",
            Self::RunOwnership => "Installation activation requires its live run owner",
            Self::Retirement { .. } => "Installation selection and protection retirement failed",
        })
    }
}
impl Error for SelectionFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(cause) | Self::RunBusy(cause) => Some(cause),
            Self::Record(cause) => Some(cause),
            Self::LockTimeout
            | Self::LockCancelled
            | Self::Superseded
            | Self::Changed
            | Self::Missing
            | Self::Revision
            | Self::RunOwnership => None,
            Self::Retirement { operation, .. } => Some(operation.as_ref()),
        }
    }
}
impl From<io::Error> for SelectionFailure {
    fn from(cause: io::Error) -> Self {
        Self::Storage(cause)
    }
}
impl From<crate::setup_storage::short_lock::Refused> for SelectionFailure {
    fn from(refused: crate::setup_storage::short_lock::Refused) -> Self {
        use crate::setup_storage::short_lock::Refused;
        match refused {
            Refused::Timeout => Self::LockTimeout,
            Refused::Cancelled => Self::LockCancelled,
            Refused::Storage(cause) => Self::Storage(cause),
        }
    }
}
impl From<Rejection> for SelectionFailure {
    fn from(cause: Rejection) -> Self {
        Self::Record(cause)
    }
}

impl From<DomainFailure<Self>> for SelectionFailure {
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
