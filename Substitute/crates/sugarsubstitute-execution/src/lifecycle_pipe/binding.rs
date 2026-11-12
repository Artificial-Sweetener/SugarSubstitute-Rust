// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Distinguish complete admitted transport bindings and actual kernel observations.
use super::*;
use sugarsubstitute_contracts::{
    backend_custody::BackendCustodyKey, recovery_supervisor::RecoveryGrantKey,
};
use sugarsubstitute_contracts::{incident::DiagnosticKey, lifecycle::CoordinatorKey};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PeerBinding {
    Desktop(RunKey),
    Diagnostic(DiagnosticKey),
    BackendCustody(BackendCustodyKey),
    Recovery(RecoveryGrantKey),
}
impl PeerBinding {
    pub(crate) fn capture_target(self) -> Option<sugarsubstitute_contracts::crash::CaptureTarget> {
        use sugarsubstitute_contracts::crash::CaptureTarget;
        match self {
            Self::Desktop(run) => Some(CaptureTarget::Desktop(run)),
            Self::BackendCustody(key) => Some(CaptureTarget::BackendCustody(key)),
            Self::Diagnostic(_) | Self::Recovery(_) => None,
        }
    }

    pub(crate) fn owner(self) -> CoordinatorKey {
        match self {
            Self::Desktop(run) => run.owner,
            Self::Diagnostic(key) => key.owner(),
            Self::BackendCustody(key) => key.owner(),
            Self::Recovery(key) => key.run().owner,
        }
    }
    pub(crate) fn desktop(self) -> Option<RunKey> {
        match self {
            Self::Desktop(run) => Some(run),
            Self::Diagnostic(_) | Self::BackendCustody(_) | Self::Recovery(_) => None,
        }
    }
    pub(crate) fn diagnostic(self) -> Option<DiagnosticKey> {
        match self {
            Self::Diagnostic(key) => Some(key),
            Self::Desktop(_) | Self::BackendCustody(_) | Self::Recovery(_) => None,
        }
    }
    /// Return inert matching facts of the admitted Root-to-recovery purpose.
    pub(crate) fn recovery(self) -> Option<RecoveryGrantKey> {
        match self {
            Self::Recovery(key) => Some(key),
            _ => None,
        }
    }
    /// Return immutable custody matching facts, without process or file authority.
    pub(crate) fn backend_custody(self) -> Option<BackendCustodyKey> {
        match self {
            Self::BackendCustody(key) => Some(key),
            Self::Desktop(_) | Self::Diagnostic(_) | Self::Recovery(_) => None,
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) struct VerifiedPeer {
    pub(crate) binding: PeerBinding,
    pub(crate) process: Option<ProcessKey>,
    pub(crate) pid: u32,
    pub(crate) birth: u64,
    pub(crate) session: u32,
}
#[expect(
    clippy::large_enum_variant,
    reason = "Eight inline observations are included in ActiveState::storage_bytes; authentication adds no separate allocation"
)]
pub(crate) enum Observation {
    Prepared,
    Connected,
    Authenticated(VerifiedPeer),
    ReadPending { operation: LifecyclePipeOperationId },
    Received { operation: LifecyclePipeOperationId, frame: ReceivedLifecycleFrame },
    WriteIssued { operation: LifecyclePipeOperationId, at: Moment },
    WriteRevoked { operation: LifecyclePipeOperationId, buffer: LifecyclePipeWriteBuffer },
    Written { operation: LifecyclePipeOperationId, bytes: u32 },
}
impl Observation {
    pub(crate) fn desktop(self) -> Option<LifecyclePipeObservation> {
        Some(match self {
            Self::Prepared => LifecyclePipeObservation::Prepared,
            Self::Connected => LifecyclePipeObservation::Connected,
            Self::Authenticated(peer) => {
                LifecyclePipeObservation::Authenticated(VerifiedLifecyclePeer {
                    run: peer.binding.desktop()?,
                    process: peer.process,
                    pid: peer.pid,
                    birth: peer.birth,
                    session: peer.session,
                })
            }
            Self::ReadPending { operation } => LifecyclePipeObservation::ReadPending { operation },
            Self::Received { operation, frame } => {
                LifecyclePipeObservation::Received { operation, frame }
            }
            Self::WriteIssued { operation, at } => {
                LifecyclePipeObservation::WriteIssued { operation, at }
            }
            Self::WriteRevoked { operation, buffer } => {
                LifecyclePipeObservation::WriteRevoked { operation, buffer }
            }
            Self::Written { operation, bytes } => {
                LifecyclePipeObservation::Written { operation, bytes }
            }
        })
    }
    pub(crate) fn diagnostic(self) -> Option<crate::diagnostic_pipe::DiagnosticPipeObservation> {
        use crate::diagnostic_pipe::{DiagnosticPipeObservation as Out, VerifiedDiagnosticPeer};
        Some(match self {
            Self::Prepared => Out::Prepared,
            Self::Connected => Out::Connected,
            Self::Authenticated(peer) => Out::Authenticated(VerifiedDiagnosticPeer {
                key: peer.binding.diagnostic()?,
                process: peer.process,
                pid: peer.pid,
                birth: peer.birth,
                session: peer.session,
            }),
            Self::ReadPending { operation } => Out::ReadPending { operation },
            Self::Received { operation, frame } => Out::Received { operation, frame },
            Self::WriteIssued { operation, at } => Out::WriteIssued { operation, at },
            Self::WriteRevoked { operation, buffer } => Out::WriteRevoked { operation, buffer },
            Self::Written { operation, bytes } => Out::Written { operation, bytes },
        })
    }
    /// Project only the actual admitted custody session's bounded transport facts.
    pub(crate) fn backend_custody(
        self,
    ) -> Option<crate::backend_custody::BackendCustodyPipeObservation> {
        use crate::backend_custody::{
            BackendCustodyPipeObservation as Out, VerifiedBackendCustodyPeer,
        };
        Some(match self {
            Self::Prepared => Out::Prepared,
            Self::Connected => Out::Connected,
            Self::Authenticated(peer) => Out::Authenticated(VerifiedBackendCustodyPeer {
                key: peer.binding.backend_custody()?,
                process: peer.process,
                pid: peer.pid,
                birth: peer.birth,
                session: peer.session,
            }),
            Self::ReadPending { operation } => Out::ReadPending { operation },
            Self::Received { operation, frame } => Out::Received { operation, frame },
            Self::WriteIssued { operation, at } => Out::WriteIssued { operation, at },
            Self::WriteRevoked { operation, buffer } => Out::WriteRevoked { operation, buffer },
            Self::Written { operation, bytes } => Out::Written { operation, bytes },
        })
    }
    /// Project only the actual admitted custody session's bounded transport facts.
    pub(crate) fn recovery(self) -> Option<crate::recovery_supervisor::RecoveryPipeObservation> {
        use crate::recovery_supervisor::{RecoveryPipeObservation as Out, VerifiedRecoveryPeer};
        Some(match self {
            Self::Prepared => Out::Prepared,
            Self::Connected => Out::Connected,
            Self::Authenticated(peer) => Out::Authenticated(VerifiedRecoveryPeer {
                key: peer.binding.recovery()?,
                process: peer.process,
                pid: peer.pid,
                birth: peer.birth,
                session: peer.session,
            }),
            Self::ReadPending { operation } => Out::ReadPending { operation },
            Self::Received { operation, frame } => Out::Received { operation, frame },
            Self::WriteIssued { operation, at } => Out::WriteIssued { operation, at },
            Self::WriteRevoked { operation, buffer } => Out::WriteRevoked { operation, buffer },
            Self::Written { operation, bytes } => Out::Written { operation, bytes },
        })
    }
}
