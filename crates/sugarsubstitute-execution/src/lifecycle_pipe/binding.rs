// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Distinguish complete admitted transport bindings and actual kernel observations.
use super::*;
use sugarsubstitute_contracts::{incident::DiagnosticKey, lifecycle::CoordinatorKey};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PeerBinding {
    Desktop(RunKey),
    Diagnostic(DiagnosticKey),
}
impl PeerBinding {
    pub(crate) fn owner(self) -> CoordinatorKey {
        match self {
            Self::Desktop(run) => run.owner,
            Self::Diagnostic(key) => key.owner(),
        }
    }
    pub(crate) fn desktop(self) -> Option<RunKey> {
        match self {
            Self::Desktop(run) => Some(run),
            Self::Diagnostic(_) => None,
        }
    }
    pub(crate) fn diagnostic(self) -> Option<DiagnosticKey> {
        match self {
            Self::Diagnostic(key) => Some(key),
            Self::Desktop(_) => None,
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
}
