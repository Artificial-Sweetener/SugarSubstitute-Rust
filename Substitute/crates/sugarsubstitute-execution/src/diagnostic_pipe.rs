// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own typed diagnostic sessions through the one admitted native pipe worker.
//! Crash coordination owns command codecs and replay; supervision owns helper process settlement.
mod control;
mod types;
pub use crate::lifecycle_pipe::{
    LifecycleClientCapability, LifecyclePipeDeadline, LifecyclePipeError, LifecyclePipeFailure,
    LifecyclePipeOperationId, LifecyclePipeWriteBuffer, LifecyclePipeWriteRejection,
    MAX_LIFECYCLE_MESSAGE_BYTES, MIN_LIFECYCLE_PIPE_BYTES, NativeWriteAdmission,
    NativeWriteRevocation, ReceivedLifecycleFrame,
};
pub use control::{DiagnosticPipeControl, DiagnosticPipeObservations, DiagnosticPipeSession};
pub use types::{DiagnosticPipeObservation, DiagnosticPipeReport, VerifiedDiagnosticPeer};
