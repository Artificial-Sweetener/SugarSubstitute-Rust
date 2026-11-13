// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain authenticated recovery transport and its genuine Root grant custody.
//! The existing Root owns all coordinator/backend/profile policy. This transport
//! cannot repair any prior process registration or transfer coordinator authority.
mod control;
pub(crate) mod root_desktop;
mod sending;
pub(crate) mod startup;
pub use startup::{
    AuthenticatedRecoveryOrigin, RecoveryStartup, RecoveryStartupAssembly, RecoveryStartupError,
};
mod types;
pub use crate::lifecycle_pipe::{
    LifecycleClientCapability, LifecyclePipeDeadline, LifecyclePipeError, LifecyclePipeFailure,
    LifecyclePipeOperationId, LifecyclePipeWriteBuffer, LifecyclePipeWriteRejection,
    MAX_LIFECYCLE_MESSAGE_BYTES, MIN_LIFECYCLE_PIPE_BYTES, NativeWriteAdmission,
    NativeWriteRevocation, ReceivedLifecycleFrame,
};
pub use control::{
    RecoveryPipeControl, RecoveryPipeObservations, RecoveryPipeRetirement, RecoveryPipeSession,
};
pub use types::{RecoveryPipeObservation, RecoveryPipeReport, VerifiedRecoveryPeer};

mod enrollment;
pub use enrollment::RecoveryDesktopEnrollment;
