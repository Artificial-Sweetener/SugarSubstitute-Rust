// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain which production child consumer step returned a delivery error.
use std::{error::Error, fmt};

/// The production child operation whose error ended its completion delivery.
///
/// The step is authored by the child at the call that failed, so it names the failing
/// operation even when the cause itself is an invariant with no closed category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupChildStep {
    /// Bootstrap capture, transport and configuration consumption.
    Bootstrap,
    /// The original acquisition cutoff check.
    AcquisitionExpiry,
    /// Main window request completion.
    Main,
    /// Managed output parser completion.
    Output,
    /// Lifecycle pipe observations, replies and UI watchdog challenges.
    Channel,
    /// Retiring output after its admission closed.
    OutputRetirement,
    /// Shutdown control and retirement.
    Shutdown,
    /// Failed-attempt cleanup.
    AttemptCleanup,
    /// Backend acquisition request and reply.
    Acquisition,
    /// Connection attempt progress.
    Attempt,
    /// Lifecycle control requests such as native activity notifications.
    Control,
    /// Browser profile reply transfer.
    Profile,
    /// Managed output pulls.
    OutputPull,
    /// The SugarCubes panel handshake and heartbeat.
    Panel,
    /// Retaining a failed attempt's cause.
    AttemptRetention,
    /// The child's preparation diagnostics.
    Diagnostics,
}

/// Own the unchanged child error and the step that returned it.
pub struct StartupChildFailure {
    step: StartupChildStep,
    cause: Box<dyn Error>,
}

impl StartupChildFailure {
    /// Attach the failing step; an error that already names its step keeps the innermost one.
    pub fn retain(step: StartupChildStep, cause: Box<dyn Error>) -> Box<dyn Error> {
        if cause.downcast_ref::<Self>().is_some() {
            return cause;
        }
        Box::new(Self { step, cause })
    }

    /// Read the step of a directly retained child failure without walking other sources.
    pub fn step_of(cause: &(dyn Error + 'static)) -> Option<StartupChildStep> {
        cause.downcast_ref::<Self>().map(|failure| failure.step)
    }

    /// Return the step whose call returned the error.
    pub const fn step(&self) -> StartupChildStep {
        self.step
    }

    /// Borrow the same original cause without invoking its source callback.
    pub fn original(&self) -> &(dyn Error + 'static) {
        self.cause.as_ref()
    }
}

impl fmt::Debug for StartupChildFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StartupChildFailure({:?})", self.step)
    }
}

impl fmt::Display for StartupChildFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Production child {:?} step failed", self.step)
    }
}

impl Error for StartupChildFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

#[cfg(test)]
#[path = "child_step_tests.rs"]
mod tests;
