// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve selection/domain failures alongside native protection retirement.
use super::protection::DomainFailure;
use std::{error::Error, fmt, io};
use sugarsubstitute_contracts::installation::Rejection;

/// Preserve storage and domain causes while distinguishing stale writers from corruption.
#[derive(Debug)]
pub enum SelectionFailure {
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
            Self::Storage(cause) => Some(cause),
            Self::Record(cause) => Some(cause),
            Self::Changed | Self::Missing | Self::Revision | Self::RunOwnership => None,
            Self::Retirement { operation, .. } => Some(operation.as_ref()),
        }
    }
}
impl From<io::Error> for SelectionFailure {
    fn from(cause: io::Error) -> Self {
        Self::Storage(cause)
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
