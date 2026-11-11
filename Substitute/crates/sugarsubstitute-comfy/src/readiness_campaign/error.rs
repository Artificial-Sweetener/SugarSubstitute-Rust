// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve campaign rejection and original boundary failures as typed causes.
use crate::readiness_transport::ReadinessTransportError;
use std::{error::Error, fmt};
use sugarsubstitute_application::startup::ProbeBudgetError;
use sugarsubstitute_contracts::execution::{
    AdmissionError, ExecutionError, PanicCause, TerminalKind, TimeError,
};

/// Reject an immediate campaign request before new external effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CampaignRejection {
    /// Target ownership, literal origin or effective launch inputs do not match.
    WrongTarget,
    /// Current attempts, cleanup, readiness or terminal state prohibit this request.
    WrongPhase,
    /// A supplied clock precedes the last accepted moment.
    ClockRegression,
    /// The supplied scope belongs to another execution lifetime.
    ForeignExecution,
    /// Scope cancellation or closure prohibits new work.
    ClosedScope,
    /// The finite job deadline exceeds an original admitted boundary.
    InvalidSpecification,
}

/// Retain a typed campaign failure without classifying diagnostic text.
#[derive(Debug)]
pub enum CampaignError {
    /// Immediate request ordering or binding rejection.
    Rejected(CampaignRejection),
    /// Original campaign interval does not admit another attempt.
    Budget(ProbeBudgetError),
    /// Checked backoff arithmetic failed.
    Time(TimeError),
    /// The existing execution clock failed.
    Clock(ExecutionError),
    /// Actual execution admission rejected work before its factory ran.
    Admission(AdmissionError),
    /// Original HTTP, source, native identity or cleanup failure.
    Transport(ReadinessTransportError),
    /// Actual cleanup stopped with a non-success terminal outcome.
    CleanupOutcome(TerminalKind),
    /// Preserve the actual execution owner's bounded cleanup panic payload.
    CleanupPanicked(PanicCause),
}
impl fmt::Display for CampaignError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(reason) => write!(f, "Readiness campaign rejected: {reason:?}"),
            Self::Budget(cause) => cause.fmt(f),
            Self::Time(cause) => cause.fmt(f),
            Self::Clock(cause) => cause.fmt(f),
            Self::Admission(cause) => cause.fmt(f),
            Self::Transport(cause) => cause.fmt(f),
            Self::CleanupOutcome(kind) => write!(f, "Readiness cleanup ended: {kind:?}"),
            Self::CleanupPanicked(_) => f.write_str("Readiness cleanup panicked"),
        }
    }
}
impl Error for CampaignError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Rejected(_) | Self::CleanupOutcome(_) | Self::CleanupPanicked(_) => None,
            Self::Budget(cause) => Some(cause),
            Self::Time(cause) => Some(cause),
            Self::Clock(cause) => Some(cause),
            Self::Admission(cause) => Some(cause),
            Self::Transport(cause) => Some(cause),
        }
    }
}
