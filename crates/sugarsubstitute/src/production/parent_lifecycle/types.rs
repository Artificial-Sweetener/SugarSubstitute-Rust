// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow genuine composition owners and retain typed request continuations.
use super::super::coordinator_backend::{BackendPreparation, BackendResources, CoordinatorBackend};
use std::{fmt, num::NonZeroU32};
use sugarsubstitute_contracts::{
    execution::{JobId, Moment, Scope},
    lifecycle::{BackendTarget, LeaseKey, LifecycleFailureCode, ProcessKey, UiChallengeKey},
    startup::MainIdentity,
};
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    LifecyclePipeSession, NativeWriteAdmission,
};
use sugarsubstitute_supervision::profiles::WindowsProfileLease;

/// Actual borrowed parent owners. The activation keeps the profile/session and
/// original projection alive; this router owns consumption of their pipe result.
pub(crate) struct ParentResources<'a, 'b> {
    pub(crate) session: &'a mut LifecyclePipeSession,
    pub(crate) process: ProcessKey,
    pub(crate) profile: &'a mut WindowsProfileLease,
    pub(crate) requested_profile: &'a [u16],
    pub(crate) target: &'a BackendTarget,
    /// The actual operation parent scope, independent of completed projection custody.
    /// Each submitted profile job has its retained finite frontend cutoff.
    pub(crate) finite: &'a Scope,
    pub(crate) backend: &'a mut CoordinatorBackend,
    pub(crate) preparation: &'a mut Option<BackendPreparation>,
    pub(crate) execution: &'a mut BackendResources<'b>,
}

/// Categorical public errors; original native/codec/provider causes stay in the actor.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ParentError {
    Storage,
    WrongPhase,
    ChangedDuplicate,
    StaleRequest,
    Overloaded,
    ProfileMismatch,
    ForeignPeer,
    IdentityExhausted,
    OwnerFailed,
    /// Original failure of this exact child pipe after actual checked native terminal adoption.
    DesktopStopped,
}
impl fmt::Display for ParentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Parent lifecycle rejected: {self:?}")
    }
}
impl std::error::Error for ParentError {}

/// Classify this actual activity turn at its owning boundary, independently of exit facts.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ParentActivityFailureOrigin {
    Coordinator,
    DesktopPipeTerminal,
}

#[derive(Clone, Copy)]
pub(super) enum Reply {
    Acknowledged,
    Folder { identity: MainIdentity, next: u32 },
    Environment(MainIdentity),
    Browser { identity: MainIdentity, pid: NonZeroU32 },
    Granted { lease: LeaseKey, identity: MainIdentity },
    Released,
    Reloaded(LeaseKey),
    Cancelled,
    Failed(LifecycleFailureCode),
    Rejected(sugarsubstitute_contracts::lifecycle::LifecycleRejection),
    Output(super::output::OutputFacts),
    OutputIdle(LeaseKey),
    MainFailed(sugarsubstitute_supervision::lifecycle_protocol::MainStageFailure),
}

pub(super) enum WritePurpose {
    Welcome,
    Reply(usize),
    AcquisitionProgress(sugarsubstitute_supervision::lifecycle_protocol::WireHeader),
    Challenge(UiChallengeKey),
}
pub(super) struct PendingWrite {
    pub(super) receipt: super::write_receipt::WriteReceipt,
    pub(super) admission: NativeWriteAdmission,
    pub(super) purpose: WritePurpose,
}

pub(super) struct ProfilePending {
    pub(super) job: JobId,
    pub(super) index: usize,
    pub(super) identity: MainIdentity,
    pub(super) pid: Option<NonZeroU32>,
    pub(super) original: Moment,
}
