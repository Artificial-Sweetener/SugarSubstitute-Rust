// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry startup observations without process handles, schedulers or native authority.

use super::{LaunchId, ReadinessReceipt};
use crate::{configuration::LoopbackOrigin, execution::Generation};
use std::{error::Error, fmt};

/// Classify a startup adapter failure independently of localized presentation copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupFailureCode {
    /// Trust validation or connection preparation failed.
    ConnectionPreparation,
    /// The requested backend could not be reached or proved ready.
    ConnectionUnavailable,
    /// The supervised child exited before startup finished.
    ProcessExited,
    /// An application-owned startup deadline expired.
    TimedOut,
    /// A required native presentation or handoff operation failed.
    Presentation,
}

/// Own one original adapter cause; diagnostics do not clone or automatically log it.
///
/// Native causes may own opaque adapter data. This is not an untrusted wire
/// payload or an allocation-budget claim. Product copy uses the classification,
/// while explicit diagnostics can inspect the preserved source.
/// The cause remains on its owning thread; this value does not require `Send`.
/// Keep native retirement owners separate from diagnostic causes. Execution jobs
/// return their own transferable errors; compose this failure after owned delivery.
#[derive(Debug)]
pub struct StartupFailure {
    code: StartupFailureCode,
    cause: Box<dyn Error>,
}

impl StartupFailure {
    /// Preserve an already owned adapter error without stringifying its source.
    pub fn new(code: StartupFailureCode, cause: Box<dyn Error>) -> Self {
        Self { code, cause }
    }

    /// Return the stable classification; it confers no cleanup authority.
    pub const fn code(&self) -> StartupFailureCode {
        self.code
    }
}

impl fmt::Display for StartupFailure {
    /// Redact potentially sensitive adapter diagnostics from ordinary display.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Startup adapter failure: {:?}", self.code)
    }
}

impl Error for StartupFailure {
    /// Retain the original typed cause for explicit diagnostics.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

/// Describe one observation from an authoritative startup boundary.
///
/// These values report facts; they do not prove origin authentication,
/// filesystem trust or process ownership. The emitting adapter establishes
/// those guarantees before sending an observation to the application.
/// Failure observations can retain thread-affine diagnostic causes. Deliver them
/// on the application owner thread rather than submitting them as execution jobs.
#[derive(Debug)]
pub enum StartupObservation {
    /// Native presentation and control submission actually succeeded.
    /// Returning prepared pixels alone does not establish this fact.
    SplashPresented,
    /// The requested backend passed its bounded readiness checks.
    /// Managed launch also requires the process owner's identity evidence;
    /// attachment never grants ownership of the external backend.
    ConnectionReady(LoopbackOrigin),
    /// The requested frontend navigation was actually started.
    FrontendNavigationStarted(Generation),
    /// Origin-checked, bounded frontend evidence reached its terminal result.
    FrontendReceipt(ReadinessReceipt),
    /// Normal splash retirement actually completed before main-surface reveal.
    /// This observation does not cancel the ready connection.
    SplashRetired,
    /// The main surface's native show and required control submission succeeded.
    /// This is adapter evidence, not proof of compositor visibility or security.
    MainRevealed,
    /// The connection owner observed cleanup settlement after cancellation.
    /// Before emitting this, the composing owner also settles other resources
    /// of this attempt. Cancellation requests or a dropped future are insufficient.
    ConnectionSettled,
    /// A startup adapter failed, retaining its original owned cause.
    Failed(StartupFailure),
}

/// Tag an immutable observation with the application-owned attempt identity.
#[derive(Debug)]
pub struct StartupEvent {
    launch: LaunchId,
    observation: StartupObservation,
}

impl StartupEvent {
    /// Transfer one observation without creating resources or granting privileges.
    /// Nested frontend receipts retain their own identity and are checked separately.
    pub const fn new(launch: LaunchId, observation: StartupObservation) -> Self {
        Self { launch, observation }
    }

    /// Return the attempt claimed by the emitting owner.
    pub const fn launch(&self) -> LaunchId {
        self.launch
    }

    /// Borrow the observation without changing its attempt identity.
    pub const fn observation(&self) -> &StartupObservation {
        &self.observation
    }

    /// Transfer the observation after application ordering and identity validation.
    pub fn into_observation(self) -> StartupObservation {
        self.observation
    }
}
