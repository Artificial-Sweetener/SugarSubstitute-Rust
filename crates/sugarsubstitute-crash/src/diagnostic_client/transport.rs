// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt the sole execution-owned charged diagnostic transport without copying buffers.
use super::*;
use sugarsubstitute_contracts::execution::Moment;
use sugarsubstitute_execution::diagnostic_pipe::{
    DiagnosticPipeControl, LifecyclePipeError, LifecyclePipeOperationId, LifecyclePipeWriteBuffer,
    NativeWriteAdmission, ReceivedLifecycleFrame, VerifiedDiagnosticPeer,
};
/// Preserve original charged output on codec or actual native admission rejection.
pub struct DiagnosticSendRejection {
    /// Exact wire/native category; native error is preserved rather than erased.
    pub error: DiagnosticSendError,
    /// Original actual charged guard, returned unchanged on codec rejection.
    pub buffer: LifecyclePipeWriteBuffer,
}
/// Keep original typed error at the real native boundary.
#[derive(Debug)]
pub enum DiagnosticSendError {
    /// Pure wire encoding rejected before native issue.
    Wire(DiagnosticWireError),
    /// Existing native admission rejected, retaining its exact original error.
    Native(LifecyclePipeError),
}
impl std::fmt::Display for DiagnosticSendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Wire(e) => e.fmt(f),
            Self::Native(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for DiagnosticSendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Wire(e) => e,
            Self::Native(e) => e,
        })
    }
}
/// Encode in the original claimed native buffer and request existing native admission.
/// # Errors
/// Returns original guard on every encoding/admission rejection. Admission is not
/// native issue, completion, peer processing or a durable repository effect.
pub fn send_diagnostic(
    control: &DiagnosticPipeControl,
    operation: LifecyclePipeOperationId,
    original: Moment,
    buffer: LifecyclePipeWriteBuffer,
    frame: DiagnosticFrame<'_>,
) -> Result<NativeWriteAdmission, DiagnosticSendRejection> {
    if control.prepared_key() != Some(frame.key) {
        return Err(DiagnosticSendRejection {
            error: DiagnosticSendError::Wire(DiagnosticWireError::Identity),
            buffer,
        });
    }
    let (buffer, length) = routing::prepare(buffer, frame, LifecyclePipeWriteBuffer::as_mut_bytes)
        .map_err(|(error, buffer)| DiagnosticSendRejection {
            error: DiagnosticSendError::Wire(error),
            buffer,
        })?;
    control.write(operation, original, buffer, length).map_err(|rejection| {
        DiagnosticSendRejection {
            error: DiagnosticSendError::Native(rejection.error),
            buffer: rejection.buffer,
        }
    })
}
/// Retain complete original input loan until the composing consumer disposes it.
/// Decoded context is data, not repository/family/presentation authority.
pub struct ReceivedDiagnosticRequest {
    frame: ReceivedLifecycleFrame,
    key: sugarsubstitute_contracts::incident::DiagnosticKey,
}
/// Preserve complete input loan on origin/shape rejection.
pub struct DiagnosticReceiveRejection {
    /// Exact categorical rejection.
    pub error: DiagnosticWireError,
    /// Original execution-owned charged frame.
    pub frame: ReceivedLifecycleFrame,
}
impl ReceivedDiagnosticRequest {
    /// Correlate actual kernel-verified peer, trusted expected session and complete bytes.
    /// Caller must drain peer and frame from the SAME retained actual session;
    /// matching fields alone cannot establish that native association or durability.
    /// # Errors
    /// Foreign peer/session or malformed content returns the unchanged original loan.
    pub fn from_native(
        peer: VerifiedDiagnosticPeer,
        expected: sugarsubstitute_contracts::incident::DiagnosticKey,
        frame: ReceivedLifecycleFrame,
    ) -> Result<Self, DiagnosticReceiveRejection> {
        routing::receive(peer.key(), expected, frame, ReceivedLifecycleFrame::as_bytes)
            .map(|frame| Self { frame, key: expected })
            .map_err(|(error, frame)| DiagnosticReceiveRejection { error, frame })
    }
    /// Borrow current complete bytes without copying or releasing their real claim.
    pub fn bytes(&self) -> &[u8] {
        self.frame.as_bytes()
    }
    /// Borrow strict typed request while the original charged input loan stays alive.
    /// # Errors
    /// Retains a strict codec rejection rather than fabricating a request.
    pub fn decode(&self) -> Result<DiagnosticFrame<'_>, DiagnosticWireError> {
        decode(self.frame.as_bytes())
    }
    /// Observe exact expected session only; no OS or store privilege is exposed.
    pub const fn key(&self) -> sugarsubstitute_contracts::incident::DiagnosticKey {
        self.key
    }
}
