// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Order one startup attempt without owning processes, native surfaces or execution.
//!
//! The host delivers identity-tagged observations from authoritative adapters and
//! consumes at most one effect per transition. Native first-frame success precedes
//! connection preparation; backend readiness precedes frontend readiness. Normal
//! splash retirement preserves the connection, while cancellation closes admission
//! and requires observed settlement before an increasing-identity retry.

mod budget;
pub use budget::{ConnectionBudget, ConnectionBudgetKind, FrontendBudget, ProbeBudgetError};

mod session;
pub use session::StartupSession;
mod progress;
pub use progress::StartupProgress;

use std::{error::Error, fmt};
use sugarsubstitute_contracts::startup::{
    ProbeFailure, ReadinessFailure, StartupEvent, StartupFailure,
};

/// Identify the current acknowledged phase, independently of animation or log volume.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupPhase {
    /// Wait for a successful native splash submission before connection work.
    SplashFirstFrame,
    /// Connection preparation was requested but readiness is unproved.
    PreparingConnection,
    /// Backend readiness admitted a request for the current frontend navigation.
    ServerReady,
    /// The requested navigation started but frontend readiness is unproved.
    WebviewLoading,
    /// Valid frontend evidence admitted normal splash retirement.
    FrontendReady,
    /// Retirement was acknowledged but main-surface reveal remains unacknowledged.
    RevealingMain,
    /// Main-surface native show and required control submission were acknowledged.
    Running,
    /// A terminal startup failure closed admission and requested cleanup.
    Failed,
    /// Cancellation closed admission but actual cleanup settlement is outstanding.
    Cancelling,
    /// Cleanup settlement was observed; a fresh attempt may now be requested.
    Stopped,
}

/// Reject an input without changing phase, identity, readiness or the retained failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupRejection {
    /// The input belongs to a different launch attempt.
    ForeignLaunch,
    /// An acknowledgement arrived outside the phase where it was requested.
    InvalidOrder,
    /// The endpoint does not match the configured attachment or managed port.
    WrongOrigin,
    /// A callback claims an earlier or unrequested future navigation.
    WrongNavigation,
    /// Frontend completion already consumed its one terminal allowance.
    AlreadyCompleted,
    /// Cancellation or failure permanently closed frontend admission.
    AdmissionClosed,
    /// Retry cannot reuse resources until authoritative settlement is observed.
    SettlementRequired,
    /// Retry identities must strictly increase; zero is excluded by the identity type.
    LaunchIdentityNotIncreasing,
}

impl fmt::Display for StartupRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Rejected startup transition: {self:?}")
    }
}

impl Error for StartupRejection {}

/// Return a rejected observation to its caller, including any original adapter cause.
#[derive(Debug)]
pub struct RejectedStartupEvent {
    reason: StartupRejection,
    event: StartupEvent,
}

impl RejectedStartupEvent {
    /// Return the ordering or identity rejection without consuming the observation.
    pub const fn reason(&self) -> StartupRejection {
        self.reason
    }
    /// Borrow the rejected event; unexpected adapter causes remain owned here.
    pub const fn event(&self) -> &StartupEvent {
        &self.event
    }
    /// Return ownership to the emitting boundary for explicit disposal or diagnostics.
    pub fn into_event(self) -> StartupEvent {
        self.event
    }
}

impl fmt::Display for RejectedStartupEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.reason.fmt(formatter)
    }
}

impl Error for RejectedStartupEvent {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self.event.observation() {
            sugarsubstitute_contracts::startup::StartupObservation::Failed(failure) => {
                Some(failure)
            }
            _ => Some(&self.reason),
        }
    }
}

/// Retain the sole accepted startup failure; later observations cannot replace it.
#[derive(Debug)]
pub enum SessionFailure {
    /// Original typed adapter cause, owned without cloning or stringification.
    Adapter(StartupFailure),
    /// Bounded diagnostic transferred from a validated current frontend receipt.
    Frontend(ProbeFailure),
}

impl SessionFailure {
    /// Whether the page reported SugarCubes itself unavailable or failed. Only this counts
    /// against a just-installed SugarCubes; a frontend that never became usable or a probe
    /// that ran out of time may have other causes and give no verdict.
    pub fn sugarcubes_failed(&self) -> bool {
        matches!(
            self,
            Self::Frontend(failure) if matches!(
                failure.code,
                ReadinessFailure::SugarcubesUnavailable | ReadinessFailure::SugarcubesFailed
            )
        )
    }
}

impl fmt::Display for SessionFailure {
    /// Display classifications rather than untrusted details or sensitive adapter data.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Adapter(failure) => failure.fmt(formatter),
            Self::Frontend(failure) => {
                write!(formatter, "Frontend startup failure: {:?}", failure.code)
            }
        }
    }
}

impl Error for SessionFailure {
    /// Preserve native adapter sources; probe detail is separately accessible as data.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Adapter(failure) => Some(failure),
            Self::Frontend(_) => None,
        }
    }
}

#[cfg(test)]
mod session_tests;
