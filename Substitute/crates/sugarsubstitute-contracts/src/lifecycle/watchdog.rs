// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe native UI-loop evidence separately from requests to transmit a challenge.
use super::{IdentityError, RunKey};
use crate::execution::Moment;
use std::{num::NonZeroU64, time::Duration};

/// Interval between eligible native UI challenges; no background timer is created.
pub const UI_CHALLENGE_CADENCE: Duration = Duration::from_secs(5);
/// Initial suspicion interval measured from actual native dispatch, not queue admission.
pub const UI_SUSPICION_THRESHOLD: Duration = Duration::from_secs(30);

/// Positive, nonreused challenge identity within one desktop run.
/// This number grants no authentication or native privilege.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiChallengeId(NonZeroU64);
impl UiChallengeId {
    /// Reject zero rather than representing an unassigned challenge.
    pub const fn new(value: u64) -> Result<Self, IdentityError> {
        match NonZeroU64::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(IdentityError::Zero),
        }
    }
    /// Return the exact per-run matching fact.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
    /// Advance without wrapping or reusing a retired challenge.
    pub fn next(self) -> Result<Self, IdentityError> {
        Self::new(self.get().checked_add(1).ok_or(IdentityError::Exhausted)?)
    }
}
/// Bind native evidence to the exact coordinator epoch and desktop generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiChallengeKey {
    /// Current desktop lifetime, independently authenticated by its adapter.
    pub run: RunKey,
    /// Nonreused native UI challenge within that lifetime.
    pub id: UiChallengeId,
}
/// Native eligibility and acknowledged OS suspension; page state is not evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiWatchdogActivity {
    /// The desktop is visible and active, eligible for native UI-loop monitoring.
    VisibleActive,
    /// Native visibility/inactivity makes UI monitoring inapplicable.
    Inactive,
    /// The native adapter acknowledged an OS sleep or modal suspension.
    Suspended,
}
/// Trusted native facts, delivered using a separately authenticated run binding.
/// Worker heartbeats, backend silence and page JavaScript cannot produce these facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiWatchdogObservation {
    /// Actual native post/admission succeeded for a previously validated queued key.
    /// `at` is the actual dispatch time in the composing owner's monotonic epoch,
    /// not the later supervisor receipt time. The adapter serializes transmission
    /// against activity invalidation, and orders this fact before any early UI reply.
    Dispatched {
        /// Exact admitted native challenge.
        challenge: UiChallengeKey,
        /// Actual native dispatch timestamp, never a wall-clock value.
        at: Moment,
    },
    /// The actual desktop UI event loop processed the exact native challenge.
    Responded {
        /// Matching dispatched challenge, not a claimed page or worker identity.
        challenge: UiChallengeKey,
    },
}
/// At most one effect from a policy transition; neither variant performs native I/O.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiWatchdogEffect {
    /// Request one native challenge; queueing this effect proves no UI progress.
    /// The adapter must revalidate the key immediately before serialized transmission.
    Challenge {
        /// Exact current queued challenge.
        challenge: UiChallengeKey,
    },
    /// Request diagnostics and user-controlled recovery for the suspected desktop.
    /// This grants no kill, restart, backend action or inference replay authority.
    Unresponsive {
        /// Dispatched challenge whose original suspicion interval elapsed.
        challenge: UiChallengeKey,
    },
}
