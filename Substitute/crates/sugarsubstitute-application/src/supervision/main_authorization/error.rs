// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain rejected observation ownership and original clock/native source causes.
use super::MainAuthorizationObservation;
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{execution::TimeError, lifecycle::LifecycleRejection};

/// Reject ordering before mutation without converting expiry into a clock error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainAuthorizationError {
    /// Exact identity, request or stage rejection from existing lifecycle contracts.
    Rejected(LifecycleRejection),
    /// Preserve an original monotonic clock cause.
    Clock(TimeError),
    /// Ordinary work reached or passed its unchanged original frontend deadline.
    Deadline,
}
impl From<LifecycleRejection> for MainAuthorizationError {
    fn from(value: LifecycleRejection) -> Self {
        Self::Rejected(value)
    }
}
impl fmt::Display for MainAuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Main sequence rejected: {self:?}")
    }
}
impl Error for MainAuthorizationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Rejected(source) => Some(source),
            Self::Clock(source) => Some(source),
            Self::Deadline => None,
        }
    }
}

/// Return the original rejected observation, including any owned native cause.
#[derive(Debug)]
pub struct MainAuthorizationRejected {
    /// Typed rejection; ordinary formatting contains no private path/process data.
    pub error: MainAuthorizationError,
    /// Unchanged input; a rejected failure never loses its original source.
    pub observation: MainAuthorizationObservation,
}
impl fmt::Display for MainAuthorizationRejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.error, f)
    }
}
impl Error for MainAuthorizationRejected {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}
