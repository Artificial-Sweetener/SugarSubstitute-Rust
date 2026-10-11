// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Separate decoded request data from native authenticated observations.
use super::{DecodedTarget, ReplyOutcome};
use std::fmt;
use sugarsubstitute_contracts::lifecycle::{
    BackendTarget, CompletionIntent, LeaseId, ResourceGeneration, ResourceId, ShutdownId,
    UiChallengeId,
};

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
    Acquire(&'a BackendTarget),
    /// Tag 5: release this claimed resource-generation/lease reference.
    Release(LeaseReference),
    /// Tag 6: retain this claimed lease through an authorized frontend reload.
    Reload(LeaseReference),
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
    Reply(ReplyOutcome),
}
impl WirePayload<'_> {
    pub(super) const fn kind(&self) -> u16 {
        match self {
            Self::Hello(_) => 1,
            Self::Welcome { .. } => 2,
            Self::Register => 3,
            Self::Acquire(_) => 4,
            Self::Release(_) => 5,
            Self::Reload(_) => 6,
            Self::ExitIntent { .. } => 7,
            Self::ExitCompleted(_) => 8,
            Self::UiChallenge(_) => 9,
            Self::UiResponse(_) => 10,
            Self::Reply(_) => 11,
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
    Acquire(DecodedTarget<'a>),
    /// Claimed exact lease release.
    Release(LeaseReference),
    /// Claimed exact lease retention.
    Reload(LeaseReference),
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
    Reply(ReplyOutcome),
}
