// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve owner rejection causes while keeping adapter-native errors outside decisions.
use sugarsubstitute_contracts::{
    execution::TimeError, incident::IncidentError, lifecycle::LifecycleRejection,
};

/// Reject before applying a state transition or admitting an effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncidentDecisionError {
    /// Related incident facts are invalid.
    Contract(IncidentError),
    /// Authoritative lifecycle owner rejected the run admission.
    Lifecycle(LifecycleRejection),
    /// Original monotonic arithmetic failed.
    Clock(TimeError),
    /// A subject or historical publication belongs to another namespace.
    ForeignNamespace,
    /// A scope or binding no longer matches its retained operation.
    StaleOperation,
    /// An exact identity was repeated with different content.
    ChangedDuplicate,
    /// The observed/action phase is inadmissible.
    WrongPhase,
    /// All bounded records remain retained.
    Saturated,
    /// Current logical operation IDs cannot advance.
    OperationExhausted,
    /// Reporter session IDs cannot advance.
    ReportSessionExhausted,
    /// Actual committed content differs from the requested exact content.
    MismatchedCommit,
}
impl From<IncidentError> for IncidentDecisionError {
    fn from(value: IncidentError) -> Self {
        Self::Contract(value)
    }
}
impl From<LifecycleRejection> for IncidentDecisionError {
    fn from(value: LifecycleRejection) -> Self {
        Self::Lifecycle(value)
    }
}
impl From<TimeError> for IncidentDecisionError {
    fn from(value: TimeError) -> Self {
        Self::Clock(value)
    }
}
impl std::fmt::Display for IncidentDecisionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Incident decision rejected: {self:?}")
    }
}
impl std::error::Error for IncidentDecisionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Contract(e) => Some(e),
            Self::Lifecycle(e) => Some(e),
            Self::Clock(e) => Some(e),
            _ => None,
        }
    }
}
