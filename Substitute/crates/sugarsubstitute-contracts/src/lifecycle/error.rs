// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve owned native causes without disclosing sensitive adapter diagnostics.
use std::{error::Error, fmt};
/// Describe a failure without turning diagnostics into native authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleFailureCode {
    /// Resource preparation failed before usable connection readiness.
    Preparation,
    /// Native cleanup failed or remains uncertain.
    Cleanup,
    /// An observed native process ended unexpectedly.
    UnexpectedExit,
    /// The one transaction's absolute deadline elapsed before settlement.
    Deadline,
}
/// Own an original typed cause; ordinary formatting remains redacted.
pub struct LifecycleFailure {
    code: LifecycleFailureCode,
    cause: Box<dyn Error + Send + Sync>,
}
impl LifecycleFailure {
    /// Transfer the original cause without stringifying or cloning it.
    pub fn new(code: LifecycleFailureCode, cause: Box<dyn Error + Send + Sync>) -> Self {
        Self { code, cause }
    }
    /// Return the bounded classification without exposing the cause's text.
    pub const fn code(&self) -> LifecycleFailureCode {
        self.code
    }
}
impl fmt::Debug for LifecycleFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LifecycleFailure").field("code", &self.code).finish_non_exhaustive()
    }
}
impl fmt::Display for LifecycleFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Lifecycle failure: {:?}", self.code)
    }
}
impl Error for LifecycleFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
/// Reject an input before changing authoritative state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleRejection {
    /// Namespace or owner epoch does not match.
    ForeignOwner,
    /// Run, resource, process or connection is stale or unknown.
    StaleIdentity,
    /// An existing browser profile excludes another independent run.
    ProfileContention,
    /// Native cleanup or acquisition ordering prevents this operation.
    WrongPhase,
    /// Same request ID was reused with different arguments.
    ChangedDuplicate,
    /// The bounded completed history no longer contains this earlier ID.
    StaleRequest,
    /// A declared run/resource/lease/request capacity is exhausted.
    Overloaded,
    /// A positive identity sequence cannot advance.
    IdentityExhausted,
    /// The injected monotonic clock regressed or overflowed.
    InvalidClock,
    /// Connection readiness disagrees with the admitted endpoint.
    EndpointMismatch,
}
impl fmt::Display for LifecycleRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Lifecycle input rejected: {self:?}")
    }
}
impl Error for LifecycleRejection {}
/// Return rejected observation ownership, including its original native cause.
#[derive(Debug)]
pub struct RejectedObservation<T> {
    /// Reason for rejecting the unchanged input.
    pub reason: LifecycleRejection,
    /// Original input, never silently discarded by the owner.
    pub observation: T,
}
