// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve bounded input rejection and clock causes without private request content.

use crate::{configuration::ConfigurationError, execution::TimeError};
use std::{error::Error, fmt};

/// Reject inert entry shape before retaining native resources.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryShapeError {
    /// A positive identity or exact native SID shape was invalid.
    Identity,
    /// Raw path units were empty, excessive or contained NUL.
    Path,
    /// An encoded argument section was malformed.
    Arguments,
    /// The original environment snapshot was malformed or excessive.
    Environment,
    /// Retained dynamic allocation capacities exceeded their bound.
    Capacity,
    /// The authoritative configuration owner rejected an input.
    Configuration(ConfigurationError),
    /// Stored canvas renderer text exceeded its selector bound.
    CanvasRenderer,
}
impl fmt::Display for EntryShapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Identity => "Entry identity shape rejected",
            Self::Path => "Entry path shape rejected",
            Self::Arguments => "Entry argument section rejected",
            Self::Environment => "Entry environment snapshot rejected",
            Self::Capacity => "Entry retained capacity rejected",
            Self::Configuration(_) => "Entry configuration rejected",
            Self::CanvasRenderer => "Entry canvas renderer preference rejected",
        })
    }
}
impl Error for EntryShapeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Configuration(cause) => Some(cause),
            _ => None,
        }
    }
}

/// Reject a probe without renewing its original setup ancestor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryBudgetError {
    /// Preserve the authoritative checked clock arithmetic cause.
    Clock(TimeError),
    /// The original setup budget has expired, including equality.
    Expired,
}
impl fmt::Display for EntryBudgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clock(cause) => write!(f, "Entry probe clock failed: {cause}"),
            Self::Expired => f.write_str("Entry setup budget expired"),
        }
    }
}
impl Error for EntryBudgetError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Clock(cause) => Some(cause),
            Self::Expired => None,
        }
    }
}

/// Explain why uncertain coordinator ownership forbids automatic reclaim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryOwnerDegradation {
    /// Live native identity could not be independently verified.
    IdentityUncertain,
    /// An existing record was unknown or corrupt and remains preserved.
    RecordUnknown,
    /// The derived endpoint collided with an unverified owner.
    EndpointCollision,
    /// Actual native cleanup remains unresolved.
    NativeCleanupUncertain,
}

/// Closed requester outcomes carrying no desktop or backend privilege.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryRejection {
    /// Request configuration or wire shape was invalid.
    InvalidInput,
    /// Actual profile exclusion was unavailable.
    ProfileBusy,
    /// Bounded owned capacity was unavailable.
    Capacity,
    /// The original request deadline expired.
    Deadline,
    /// Native peer or correlated challenge verification failed.
    Authentication,
    /// An existing request identity was reused with changed content.
    ChangedDuplicate,
    /// An uncached request identity is at or below the settled eviction floor.
    StaleRequest,
    /// A verified initializing owner permits a bounded discovery probe.
    RecoverableContention,
    /// Uncertain ownership must remain preserved without automatic reclaim.
    DegradedOwner(EntryOwnerDegradation),
}
