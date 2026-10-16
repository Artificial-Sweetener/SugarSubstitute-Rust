// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own authenticated backend custody transport through the admitted native pipe worker.
//! The native custody driver owns Jobs and exclusions; transport facts do not prove settlement.
mod control;
pub(crate) mod deadlines;
mod driver;
mod memory;
mod turn;
pub use turn::BackendCustodySlotTurn;
mod parent_storage;
mod permit;
pub use parent_storage::BackendCustodyParentStorage;
#[cfg(windows)]
mod native_watch;
mod start;
pub(crate) mod submission;
mod types;
pub(crate) mod worker;
pub use crate::lifecycle_pipe::{
    LifecycleClientCapability, LifecyclePipeDeadline, LifecyclePipeError, LifecyclePipeFailure,
    LifecyclePipeOperationId, LifecyclePipeWriteBuffer, LifecyclePipeWriteRejection,
    MAX_LIFECYCLE_MESSAGE_BYTES, MIN_LIFECYCLE_PIPE_BYTES, NativeWriteAdmission,
    NativeWriteRevocation, ReceivedLifecycleFrame,
};
pub use control::{
    BackendCustodyPipeControl, BackendCustodyPipeObservations, BackendCustodyPipeSession,
};
pub use driver::{BackendCustodyDriver, BackendCustodyProgress, BackendCustodyWake};
pub use memory::{BackendCustodyCleanup, BackendCustodyMemory, BackendCustodySlotMemory};
#[cfg(windows)]
pub use native_watch::BackendCustodyProcessWatch;
pub use permit::BackendCustodyWorkerPermit;
pub use start::BackendCustodyWorkerStart;
pub use submission::{BACKEND_CUSTODY_POOL_BYTES, BackendCustodyWorkerAdmissionError};
pub use types::{
    BackendCustodyPipeObservation, BackendCustodyPipeReport, VerifiedBackendCustodyPeer,
};
pub use worker::{BackendCustodyWorkerFailure, BackendCustodyWorkerReport};
