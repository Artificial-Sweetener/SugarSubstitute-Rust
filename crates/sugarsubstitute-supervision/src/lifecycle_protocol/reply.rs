// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Assign stable explicit wire tags independently of Rust enum layouts.
use super::{GrantedContext, LeaseReference, MainStageFailure, OutputDelivery, ProtocolError};
use std::num::NonZeroU32;
use sugarsubstitute_contracts::lifecycle::{LifecycleFailureCode, LifecycleRejection};
use sugarsubstitute_contracts::startup::MainIdentity;

/// Bounded wire disposition; native causes and actual cleanup ownership stay local.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReplyOutcome<'a> {
    /// Tag 1: the requested finite logical operation was acknowledged.
    Acknowledged,
    /// Tag2: nonfinal original Acquire progress reserving output access only.
    /// Retain the original request and deadline until a final disposition. This copied
    /// reservation grants neither Main/Profile readiness nor Release/Reload privileges.
    /// A managed source adopts the same transcript and delivery sequence through Grant.
    Pending(LeaseReference),
    /// Tag 3: claimed matching ready lease and current main origin/versions.
    /// Native authentication and actual retained readiness/source proof remain separate.
    Granted(GrantedContext<'a>),
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
    /// Tag9: exact pending environment stage authorization; native proof stays local.
    EnvironmentAuthorized(MainIdentity),
    /// Tag10: exact pending browser stage authorization; no serialized kernel authority.
    BrowserAuthorized {
        /// Current main attempt that received native family validation.
        identity: MainIdentity,
        /// Exact matching controller PID; retained birth/Job proof remains local.
        browser_process_id: NonZeroU32,
    },
    /// Tag11: bounded assembly progress; grants no native continuation.
    FolderChunkAccepted {
        /// Current main folder transaction.
        identity: MainIdentity,
        /// Positive next offset/complete total, at most32766 units; root matches actual assembly.
        next_offset_units: u32,
    },
    /// Tag12: one borrowed raw output batch, without ingestion or process-settlement inference.
    OutputDelivery(OutputDelivery<'a>),
    /// Tag13: no currently available batch; never synthetic EOF.
    OutputIdle(LeaseReference),
    /// Tag14: bounded authorization/native stage failure; custody remains separately owned.
    MainStageFailed(MainStageFailure),
    /// Tag15: copied managed outage state answering the original OutputPull.
    /// Native cleanup, retained cause custody and retry admission remain separate.
    BackendUnavailable {
        /// Exact unavailable lease under this frame's run and connection.
        failed: LeaseReference,
        /// Complete unsigned actual root status; 259 is preserved.
        status: u32,
        /// Distinct affected Main runs, bounded to 1..64.
        affected_clients: u16,
        /// Copied actual cleanup/cause-transfer eligibility, never retry authority.
        retry_ready: bool,
    },
    /// Tag16: one actual historical Ready event answering pending RetryBackend or OutputPull.
    /// Changes only backend binding; a successor may already have an exact retained managed death.
    /// The child keeps recovery disabled until its current ordinary output/idle response.
    /// Its next authenticated current-lease OutputPull acknowledges handoff consumption;
    /// the parent independently validates live adoption or exact new-death/absent-old state.
    /// The owning request emits one response. Written alone never establishes adoption.
    BackendReplaced {
        /// Exact mounted Main identity, reconstructed with the original header run.
        identity: MainIdentity,
        /// Previously unavailable binding, retained for exact lineage validation.
        previous: LeaseReference,
        /// Genuine successor interest that actually reached Ready; not a current readiness grant.
        current: LeaseReference,
    },
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
