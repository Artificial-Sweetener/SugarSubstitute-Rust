// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain typed original preview causes and their closed observational category.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::{ExecutionError, IdentityError};
use sugarsubstitute_desktop::DesktopError;
/// Preserve qualification rejection independently of ordinary product startup health.
#[derive(Debug)]
pub(in crate::production::child) enum PreviewError {
    State,
    Identity,
    Deadline,
    Capacity,
    Execution(ExecutionError),
    Admission(sugarsubstitute_execution::BlockingInputError),
    JobIdentity(IdentityError),
}
impl fmt::Display for PreviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Native preview qualification rejected")
    }
}
impl Error for PreviewError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Execution(cause) => Some(cause),
            Self::Admission(cause) => Some(cause),
            Self::JobIdentity(cause) => Some(cause),
            _ => None,
        }
    }
}
impl PreviewError {
    pub(in crate::production::child) fn trace_cause(
        &self,
    ) -> sugarsubstitute_desktop::qualification_trace::Cause {
        use sugarsubstitute_desktop::qualification_trace::Cause;
        match self {
            Self::State => Cause::State,
            Self::Identity => Cause::Identity,
            Self::Deadline => Cause::Deadline,
            Self::Capacity => Cause::Capacity,
            Self::Execution(_) => Cause::Execution,
            Self::Admission(_) => Cause::Admission,
            Self::JobIdentity(_) => Cause::JobIdentity,
        }
    }

    pub(super) fn kind(&self) -> &'static str {
        match self {
            Self::State => "State",
            Self::Identity => "Identity",
            Self::Deadline => "Deadline",
            Self::Capacity => "Capacity",
            Self::Execution(_) => "Execution",
            Self::Admission(_) => "Admission",
            Self::JobIdentity(_) => "JobIdentity",
        }
    }
}

/// Retain the first typed original cause without an additional erased heap allocation.
pub(super) enum PreviewFailure {
    Native(DesktopError),
    Host(PreviewError),
}
impl fmt::Display for PreviewFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native(cause) => fmt::Display::fmt(cause, formatter),
            Self::Host(cause) => fmt::Display::fmt(cause, formatter),
        }
    }
}
impl PreviewFailure {
    pub(super) fn kind(&self) -> &'static str {
        match self {
            Self::Native(_) => "Native",
            Self::Host(cause) => cause.kind(),
        }
    }
}
