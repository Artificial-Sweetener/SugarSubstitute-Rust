// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Assign stable explicit wire tags independently of Rust enum layouts.
use super::{LeaseReference, ProtocolError};
use sugarsubstitute_contracts::lifecycle::{LifecycleFailureCode, LifecycleRejection};

/// Bounded wire disposition; native causes and actual cleanup ownership stay local.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplyOutcome {
    /// Tag 1: the requested finite logical operation was acknowledged.
    Acknowledged,
    /// Tag 2: lease reservation exists but readiness has not been established.
    Pending(LeaseReference),
    /// Tag 3: actual readiness established the matching lease.
    Granted(LeaseReference),
    /// Tag 4: lease release recorded; family settlement is independent.
    Released,
    /// Tag 5: matching lease retained through frontend reload.
    Reloaded(LeaseReference),
    /// Tag 6: authoritative cancellation outcome; no native clean-stop inference.
    Cancelled,
    /// Tag 7: bounded failure classification; original cause remains locally owned.
    Failed(LifecycleFailureCode),
    /// Tag 8: unchanged input rejected by the authoritative application owner.
    Rejected(LifecycleRejection),
}
pub(super) const fn failure_tag(code: LifecycleFailureCode) -> u8 {
    match code {
        LifecycleFailureCode::Preparation => 1,
        LifecycleFailureCode::Cleanup => 2,
        LifecycleFailureCode::UnexpectedExit => 3,
        LifecycleFailureCode::Deadline => 4,
    }
}
pub(super) fn failure(tag: u8) -> Result<LifecycleFailureCode, ProtocolError> {
    match tag {
        1 => Ok(LifecycleFailureCode::Preparation),
        2 => Ok(LifecycleFailureCode::Cleanup),
        3 => Ok(LifecycleFailureCode::UnexpectedExit),
        4 => Ok(LifecycleFailureCode::Deadline),
        _ => Err(ProtocolError::UnsupportedTag),
    }
}
pub(super) const fn rejection_tag(code: LifecycleRejection) -> u8 {
    match code {
        LifecycleRejection::ForeignOwner => 1,
        LifecycleRejection::StaleIdentity => 2,
        LifecycleRejection::ProfileContention => 3,
        LifecycleRejection::WrongPhase => 4,
        LifecycleRejection::ChangedDuplicate => 5,
        LifecycleRejection::StaleRequest => 6,
        LifecycleRejection::Overloaded => 7,
        LifecycleRejection::IdentityExhausted => 8,
        LifecycleRejection::InvalidClock => 9,
        LifecycleRejection::EndpointMismatch => 10,
    }
}
pub(super) fn rejection(tag: u8) -> Result<LifecycleRejection, ProtocolError> {
    match tag {
        1 => Ok(LifecycleRejection::ForeignOwner),
        2 => Ok(LifecycleRejection::StaleIdentity),
        3 => Ok(LifecycleRejection::ProfileContention),
        4 => Ok(LifecycleRejection::WrongPhase),
        5 => Ok(LifecycleRejection::ChangedDuplicate),
        6 => Ok(LifecycleRejection::StaleRequest),
        7 => Ok(LifecycleRejection::Overloaded),
        8 => Ok(LifecycleRejection::IdentityExhausted),
        9 => Ok(LifecycleRejection::InvalidClock),
        10 => Ok(LifecycleRejection::EndpointMismatch),
        _ => Err(ProtocolError::UnsupportedTag),
    }
}
