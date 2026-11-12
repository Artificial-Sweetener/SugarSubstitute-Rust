// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own purpose-typed helper channels through the one admitted native pipe worker.
//! Each channel serves one `Purpose` with its direction rule. Purpose owners keep their codecs:
//! crash coordination owns diagnostic commands and replay, the update contract owns attempt frames,
//! and supervision owns helper process settlement.
mod control;
mod types;
pub use crate::lifecycle_pipe::{
    LifecycleClientCapability, LifecyclePipeDeadline, LifecyclePipeError, LifecyclePipeFailure,
    LifecyclePipeOperationId, LifecyclePipeWriteBuffer, LifecyclePipeWriteRejection,
    MAX_LIFECYCLE_MESSAGE_BYTES, MIN_LIFECYCLE_PIPE_BYTES, NativeWriteAdmission,
    NativeWriteRevocation, ReceivedLifecycleFrame,
};
pub(crate) use control::apply_loan_rule;
pub use control::{PurposePipeControl, PurposePipeObservations, PurposePipeSession};
pub(crate) use types::Side;
pub use types::{
    Direction, Purpose, PurposeKey, PurposePipeObservation, PurposePipeReport, VerifiedPurposePeer,
};
