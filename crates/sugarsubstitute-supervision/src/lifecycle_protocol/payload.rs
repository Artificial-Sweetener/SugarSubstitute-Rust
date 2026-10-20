// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Separate decoded request data from native authenticated observations.
use super::{BorrowedBackendTarget, DecodedTarget, FolderFragment, MainStageFailure, ReplyOutcome};
use std::{
    fmt,
    num::{NonZeroU32, NonZeroU64},
};
use sugarsubstitute_contracts::lifecycle::{
    CompletionIntent, LeaseId, RequestId, ResourceGeneration, ResourceId, ShutdownId,
    UiChallengeId, UiWatchdogActivity,
};
use sugarsubstitute_contracts::startup::MainIdentity;

/// Borrow exactly 32 capability bytes without exposing them in diagnostics.
/// These bytes are claimed credentials; construction authenticates nothing.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CapabilityRef<'a>(&'a [u8; 32]);
impl<'a> CapabilityRef<'a> {
    /// Borrow fixed-size credential storage, owned and protected by the caller.
    pub const fn new(bytes: &'a [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Borrow credentials for the native authentication boundary only.
    /// Never log, publish to web content, or derive peer identity from these bytes.
    pub const fn bytes(self) -> &'a [u8; 32] {
        self.0
    }
}
impl fmt::Debug for CapabilityRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapabilityRef(REDACTED)")
    }
}

/// Claimed lease reference; its run/connection comes from the separately checked header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LeaseReference {
    /// Logical resource identity, never a PID or native handle.
    pub resource: ResourceId,
    /// Exact resource acquisition generation.
    pub generation: ResourceGeneration,
    /// Exact connection-bound lease identity.
    pub lease: LeaseId,
}

/// Borrow supported outbound payloads; encode validates the complete frame first.
#[derive(Debug)]
pub enum WirePayload<'a> {
    /// Tag 1: exactly 32 claimed capability bytes.
    Hello(CapabilityRef<'a>),
    /// Tag 2: positive signed Windows QPC frequency, in ticks per second.
    Welcome {
        /// Frequency established by the actual native clock owner.
        qpc_frequency: i64,
    },
    /// Tag 3: empty registration request; native enrollment owns launch/profile facts.
    Register,
    /// Tag 4: managed Unicode launch inputs or an exact loopback attachment.
    Acquire {
        /// Exact claimed main attempt; full run must match the frame header.
        identity: MainIdentity,
        /// Existing bounded managed or attached target.
        target: BorrowedBackendTarget<'a>,
    },
    /// Tag 5: release this claimed resource-generation/lease reference.
    Release(LeaseReference),
    /// Tag 6: retain this claimed lease through an authorized frontend reload.
    Reload {
        /// Claimed exact retained lease.
        lease: LeaseReference,
        /// Current claimed navigation/attempt.
        identity: MainIdentity,
    },
    /// Tag 7: authenticated intent is established separately by the channel owner.
    ExitIntent {
        /// Matching shutdown transaction identity.
        shutdown: ShutdownId,
        /// Supported expected completion attribution.
        intent: CompletionIntent,
        /// Expected actual Windows status, including 259.
        expected_code: u32,
    },
    /// Tag 8: claimed desktop teardown completion; no resource-settlement flags.
    ExitCompleted(ShutdownId),
    /// Tag 9: server-issued native UI challenge, not proof of dispatch.
    UiChallenge(UiChallengeId),
    /// Tag 10: claimed response; only actual native UI-loop processing can authorize it.
    UiResponse(UiChallengeId),
    /// Tag 11: bounded typed disposition, with no serialized native diagnostic text.
    Reply(ReplyOutcome<'a>),
    /// Tag12: canonical requested folder fragment, awaiting root native proof.
    RequestedProfileFolder(FolderFragment<'a>),
    /// Tag13: effective actual-environment folder fragment, not filesystem authority.
    EnvironmentPrepared(FolderFragment<'a>),
    /// Tag14: current about:blank controller PID, awaiting retained kernel family proof.
    ControllerPrepared {
        /// Exact current main observation.
        identity: MainIdentity,
        /// Positive actual PID claim; not a process handle or proof.
        browser_process_id: NonZeroU32,
    },
    /// Tag15: claimed checked main retirement, authenticated separately by its native owner.
    MainRetired(MainIdentity),
    /// Tag16: native eligibility claim; page state cannot establish its provenance.
    NativeActivity {
        /// Positive monotonic observation revision, admitted separately by the root.
        revision: NonZeroU64,
        /// Actual visibility/focus or acknowledged OS/modal state claimed by native adapter.
        activity: UiWatchdogActivity,
    },
    /// Tag17: one bounded output credit; acknowledgement requires actual transcript ingestion.
    OutputPull {
        /// Exact connection-bound managed output lease claim.
        lease: LeaseReference,
        /// Prior delivery actually consumed; absent requests the first credit.
        acknowledged_delivery: Option<NonZeroU64>,
    },
    /// Tag18: original bounded native stage classification; never native retirement proof.
    MainFailed(MainStageFailure),
    /// Tag19: settle the exact original acquisition before another attempt.
    SettleAttempt {
        /// Original current application attempt.
        identity: MainIdentity,
        /// Original Acquire request; never a caller-supplied resource privilege.
        acquisition: RequestId,
    },
    /// Tag20: explicit retry claim for the exact unavailable managed lease.
    /// Authentication, actual old settlement and retained target admission stay local.
    RetryBackend {
        /// Exact mounted Main; its full run must match the checked frame header.
        identity: MainIdentity,
        /// Original unavailable lease, never a new target or process privilege.
        failed: LeaseReference,
    },
}
impl WirePayload<'_> {
    pub(super) const fn kind(&self) -> u16 {
        match self {
            Self::Hello(_) => 1,
            Self::Welcome { .. } => 2,
            Self::Register => 3,
            Self::Acquire { .. } => 4,
            Self::Release(_) => 5,
            Self::Reload { .. } => 6,
            Self::ExitIntent { .. } => 7,
            Self::ExitCompleted(_) => 8,
            Self::UiChallenge(_) => 9,
            Self::UiResponse(_) => 10,
            Self::Reply(_) => 11,
            Self::RequestedProfileFolder(_) => 12,
            Self::EnvironmentPrepared(_) => 13,
            Self::ControllerPrepared { .. } => 14,
            Self::MainRetired(_) => 15,
            Self::NativeActivity { .. } => 16,
            Self::OutputPull { .. } => 17,
            Self::MainFailed(_) => 18,
            Self::SettleAttempt { .. } => 19,
            Self::RetryBackend { .. } => 20,
        }
    }
}
/// Borrow supported decoded payloads without constructing trusted adapter context.
#[derive(Debug)]
pub enum DecodedPayload<'a> {
    /// exactly 32 claimed capability bytes, redacted by their owned wrapper.
    Hello(CapabilityRef<'a>),
    /// Claimed positive Windows QPC frequency.
    Welcome {
        /// Ticks per second; native authentication still required.
        qpc_frequency: i64,
    },
    /// Empty registration data; no `ClientRegistration` is decoded.
    Register,
    /// Borrowed, fully preflighted target awaiting admitted owned conversion.
    Acquire {
        /// Shape-only main attempt reconstructed from exact frame run/launch/navigation.
        identity: MainIdentity,
        /// Borrowed target awaiting owned admitted conversion.
        target: DecodedTarget<'a>,
    },
    /// Claimed exact lease release.
    Release(LeaseReference),
    /// Claimed exact lease retention.
    Reload {
        /// Exact claimed retained lease.
        lease: LeaseReference,
        /// Current claimed main attempt/navigation.
        identity: MainIdentity,
    },
    /// Claimed shutdown intent, not a trusted `DesktopObservation`.
    ExitIntent {
        /// Claimed transaction identity.
        shutdown: ShutdownId,
        /// Supported completion attribution.
        intent: CompletionIntent,
        /// Expected Windows status without signed/truncated conversion.
        expected_code: u32,
    },
    /// Claimed transaction whose desktop teardown completed.
    ExitCompleted(ShutdownId),
    /// Claimed server challenge whose actual dispatch is a separate native fact.
    UiChallenge(UiChallengeId),
    /// Claimed response whose UI-loop provenance is a separate native fact.
    UiResponse(UiChallengeId),
    /// Bounded typed disposition; no original causes or sensitive strings decoded.
    Reply(ReplyOutcome<'a>),
    /// Canonical requested folder fragment, granting no filesystem authority.
    RequestedProfileFolder(FolderFragment<'a>),
    /// Effective environment folder fragment, awaiting complete admitted assembly/proof.
    EnvironmentPrepared(FolderFragment<'a>),
    /// Current about:blank controller matching data, never kernel family proof.
    ControllerPrepared {
        /// Exact claimed current native main.
        identity: MainIdentity,
        /// Claimed positive Windows PID.
        browser_process_id: NonZeroU32,
    },
    /// Claimed complete native main retirement, admitted only after authenticated provenance.
    MainRetired(MainIdentity),
    /// Native activity matching data, not acknowledged OS authority on its own.
    NativeActivity {
        /// Claimed monotonic native activity revision.
        revision: NonZeroU64,
        /// Claimed native eligibility/suspension.
        activity: UiWatchdogActivity,
    },
    /// One managed output credit, separate from transport Written or process settlement.
    OutputPull {
        /// Exact claimed managed output lease.
        lease: LeaseReference,
        /// Exact prior consumed delivery or absent first credit.
        acknowledged_delivery: Option<NonZeroU64>,
    },
    /// Bounded native failure classification, separate from actual retirement.
    MainFailed(MainStageFailure),
    /// Tag19: settle the exact original acquisition before another attempt.
    SettleAttempt {
        /// Original current application attempt.
        identity: MainIdentity,
        /// Original Acquire request; never a caller-supplied resource privilege.
        acquisition: RequestId,
    },
    /// Tag20: explicit retry claim for the exact unavailable managed lease.
    /// Authentication, actual old settlement and retained target admission stay local.
    RetryBackend {
        /// Exact mounted Main; its full run must match the checked frame header.
        identity: MainIdentity,
        /// Original unavailable lease, never a new target or process privilege.
        failed: LeaseReference,
    },
}
