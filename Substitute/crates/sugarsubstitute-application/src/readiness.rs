// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Admit one terminal web readiness result for the current application navigation.

use std::{error::Error, fmt};
use sugarsubstitute_contracts::{
    execution::Generation,
    startup::{LaunchId, ReadinessOutcome, ReadinessReceipt},
};

/// Reject receipts without changing current startup or native publication state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptRejection {
    /// A receipt belongs to a different launch attempt.
    ForeignLaunch,
    /// A receipt belongs to an earlier or unrequested future navigation.
    WrongNavigation,
    /// This navigation already admitted its one terminal result.
    AlreadyCompleted,
    /// Application cancellation permanently closed this launch's readiness gate.
    Cancelled,
}

/// Transfer accepted terminal evidence or an explicit unchanged-state rejection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadinessDecision {
    /// Transfer the accepted outcome to application startup policy.
    Accepted(ReadinessOutcome),
    /// Leave application readiness unchanged for this receipt.
    Rejected(ReceiptRejection),
}

/// Keep one launch's navigation and terminal admission with its authoritative owner.
///
/// This state machine owns no browser, process, clock, executor or native handles.
/// A desktop origin check and bounded protocol parsing precede `accept`; neither
/// accepted web evidence nor this gate grants filesystem or process privileges.
pub struct ReadinessGate {
    launch: LaunchId,
    navigation: Generation,
    completed: bool,
    cancelled: bool,
}

impl ReadinessGate {
    /// Open one application-assigned launch and its first requested navigation.
    pub const fn new(launch: LaunchId, navigation: Generation) -> Self {
        Self { launch, navigation, completed: false, cancelled: false }
    }

    /// Return the launch attempt admitted by this state owner.
    pub const fn launch(&self) -> LaunchId {
        self.launch
    }

    /// Return the sole navigation generation currently eligible for completion.
    pub const fn navigation(&self) -> Generation {
        self.navigation
    }

    /// Admit exactly one success or failure for the requested current generation.
    ///
    /// Rejected receipts cannot consume the terminal allowance. This preserves
    /// the next valid result when delayed callbacks race with navigation or cancel.
    pub fn accept(&mut self, receipt: ReadinessReceipt) -> ReadinessDecision {
        let rejection = if self.cancelled {
            Some(ReceiptRejection::Cancelled)
        } else if receipt.launch() != self.launch {
            Some(ReceiptRejection::ForeignLaunch)
        } else if receipt.navigation() != self.navigation {
            Some(ReceiptRejection::WrongNavigation)
        } else if self.completed {
            Some(ReceiptRejection::AlreadyCompleted)
        } else {
            None
        };
        if let Some(rejection) = rejection {
            return ReadinessDecision::Rejected(rejection);
        }
        self.completed = true;
        ReadinessDecision::Accepted(receipt.into_outcome())
    }

    /// Invalidate prior callbacks before beginning an explicitly requested reload.
    ///
    /// # Errors
    /// Rejects cancellation, repeated generations and backwards navigation;
    /// invalid transitions leave the existing gate intact.
    pub fn navigate(&mut self, generation: Generation) -> Result<(), NavigationError> {
        if self.cancelled {
            return Err(NavigationError::Cancelled);
        }
        if generation <= self.navigation {
            return Err(NavigationError::NotIncreasing);
        }
        self.navigation = generation;
        self.completed = false;
        Ok(())
    }

    /// Permanently close this launch against all delayed web completion callbacks.
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
}

/// Describe a rejected application navigation transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationError {
    /// Cancelled launches require a fresh application launch identity for retry.
    Cancelled,
    /// Reload generations must strictly increase without wrap or reuse.
    NotIncreasing,
}

impl fmt::Display for NavigationError {
    /// Describe the transition without exposing browser or native state.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Cancelled => "Readiness launch has been cancelled",
            Self::NotIncreasing => "Navigation generation must increase",
        })
    }
}

impl Error for NavigationError {}

#[cfg(test)]
#[path = "readiness_tests.rs"]
mod tests;
