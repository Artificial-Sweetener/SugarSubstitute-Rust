// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe fixed fixture protocol facts without interpreting them as process exits.
use crate::native::NativeCorrelation;
use std::{error::Error, fmt};

/// Minimum retained reservation for one four-event session, in bytes.
pub const MIN_FIXTURE_SESSION_BYTES: u64 = 16_384;

/// Observe one actual protocol milestone; taking a fact never establishes process exit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixtureObservation {
    /// Events, inheritable duplicates and both parent waits actually exist.
    Prepared,
    /// The child's Ready event was actually observed.
    Ready,
    /// Response was actually observed after this session committed Challenge.
    Responded,
}

/// Retain actual finite protocol effects after callbacks and parent handles retire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixtureSessionReport {
    /// Application correlation supplied before event creation.
    pub correlation: NativeCorrelation,
    /// A Ready callback was observed before protocol retirement.
    pub ready: bool,
    /// SetEvent(Challenge) actually succeeded.
    pub challenge_sent: bool,
    /// A Response callback followed the committed challenge.
    pub responded: bool,
    /// SetEvent(Release) actually succeeded.
    pub release_sent: bool,
}

/// Reject a protocol command without a native effect or disclosing handle values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixtureSessionError {
    /// The requested predecessor milestone has not occurred.
    NotReady,
    /// The one allowed command or transfer was already requested.
    AlreadyRequested,
    /// Cancellation, timeout or final release has closed command admission.
    Closed,
}
impl fmt::Display for FixtureSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Fixture session command rejected")
    }
}
impl Error for FixtureSessionError {}
