// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the exact HTTP and listener custody for one admitted Comfy output transfer.

mod native;
#[cfg(test)]
mod tests;

use super::{ComfyViewTarget, OutputArtifactIdentity};
use std::{
    error::Error,
    fmt,
    sync::{Mutex, TryLockError},
};
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    execution::{ExecutionId, JobClass, JobContext, MemoryClaim, Operation},
};
use sugarsubstitute_networking::Transport;
use sugarsubstitute_supervision::{
    backend_listener::{AttachedListenerLease, TcpOwnerWorkspace},
    cache::{CacheReader, RuntimeCache},
    lifecycle_protocol::BackendPeer,
};

/// Admission for native HTTP, TCP ownership, cache publication and bounded scratch.
pub const OUTPUT_TRANSFER_BYTES: u64 = 24 * 1024 * 1024;
const RETAINED_BYTES: u64 = 512 * 1024;

/// One configured origin and one parent-verified peer; these facts grant no native handles.
/// The caller retains this owner through the transfer and checked retirement.
pub struct ComfyOutputTransfer {
    origin: LoopbackOrigin,
    target: ComfyViewTarget,
    peer: BackendPeer,
    maximum: u64,
    expected: Option<OutputArtifactIdentity>,
    state: Mutex<State>,
}

struct State {
    http: Transport,
    listener: AttachedListenerLease,
    workspace: Option<TcpOwnerWorkspace>,
    claim: Option<MemoryClaim>,
    owner: Option<ExecutionId>,
    started: bool,
    closed: bool,
}

impl ComfyOutputTransfer {
    /// Prepare an inert transfer to the caller's selected, independently authorized backend.
    /// `maximum` is the caller's encoded-source byte limit and must admit the smallest image
    /// of the target's encoded format.
    ///
    /// # Errors
    /// Rejects a maximum too small to contain an image of the target's format before any
    /// native operation.
    pub fn new(
        origin: LoopbackOrigin,
        target: ComfyViewTarget,
        peer: BackendPeer,
        maximum: u64,
    ) -> Result<Self, OutputTransferError> {
        if !matches!(
            target.format(),
            super::ComfyOutputFormat::Png | super::ComfyOutputFormat::Jpeg
        ) {
            return Err(OutputTransferError::invalid("output_native_image_format"));
        }
        Self::prepare(origin, target, peer, maximum)
    }

    /// Prepare shared current-peer custody; artifact formats require the public verified constructor.
    fn prepare(
        origin: LoopbackOrigin,
        target: ComfyViewTarget,
        peer: BackendPeer,
        maximum: u64,
    ) -> Result<Self, OutputTransferError> {
        if maximum < target.format().minimum_bytes() {
            return Err(OutputTransferError::invalid("output_limit"));
        }
        Ok(Self {
            origin,
            target,
            peer,
            maximum,
            expected: None,
            state: Mutex::new(State {
                http: Transport::default(),
                listener: AttachedListenerLease::new(),
                workspace: None,
                claim: None,
                owner: None,
                started: false,
                closed: false,
            }),
        })
    }

    /// Prepare a current-peer transfer that must match a durable receipt before publication.
    /// The caller supplies an independently authorized current origin and peer; the receipt
    /// never establishes backend custody. Use the normal fetch and checked retirement lifecycle.
    /// # Errors
    /// Rejects a receipt length outside the configured source or encoded-format bounds.
    pub fn verified(
        origin: LoopbackOrigin,
        target: ComfyViewTarget,
        peer: BackendPeer,
        maximum: u64,
        expected: OutputArtifactIdentity,
    ) -> Result<Self, OutputTransferError> {
        if expected.bytes() > maximum || expected.bytes() < target.format().minimum_bytes() {
            return Err(OutputTransferError::invalid("output_identity_limit"));
        }
        let mut transfer = Self::prepare(origin, target, peer, maximum)?;
        transfer.expected = Some(expected);
        Ok(transfer)
    }

    /// Fetch and pin one complete original matching the target on an execution-owned storage worker.
    /// Content type, length and framing must match; verified artifacts also match their recorded
    /// digest before publication. Native image and browser media owners validate presentation.
    /// The connected peer is checked against the parent-granted PID and creation time
    /// before and after the bounded body. `now` is Unix UTC seconds for cache retention.
    /// The returned reader and its final release remain storage-worker responsibilities.
    /// The job must have a finite deadline, use `InfrastructureOrdinary`, and identify
    /// its operation as `Infrastructure`; lifecycle capacity remains available for shutdown.
    ///
    /// # Errors
    /// Preserves HTTP, peer, content, cache, admission and cleanup failures. After any
    /// failure, retain this owner and call `retire` on an admitted cleanup worker until settled.
    pub fn fetch(
        &self,
        cache: RuntimeCache,
        now: u64,
        context: &JobContext,
    ) -> Result<CacheReader<'static>, OutputTransferError> {
        if context.spec().class != JobClass::InfrastructureOrdinary
            || context.spec().operation != Operation::Infrastructure
            || context.cancellation_requested()
            || context.deadline().is_none_or(|deadline| context.now() >= deadline)
        {
            return Err(OutputTransferError::invalid("output_context"));
        }
        let mut state =
            self.state.lock().map_err(|_| OutputTransferError::invalid("output_owner_poisoned"))?;
        if state.started || state.closed {
            return Err(OutputTransferError::invalid("output_already_started"));
        }
        state.started = true;
        state.owner = Some(context.owner_id());
        let result = native::fetch(&mut state, self, cache, now, context);
        let cleanup = state.retire(context);
        match (result, cleanup) {
            (Ok(reader), Ok(())) => Ok(reader),
            (Err(cause), Ok(())) | (Ok(_), Err(cause)) => Err(cause),
            (Err(original), Err(cleanup)) => Err(OutputTransferError::new(
                "output_and_retirement",
                CombinedFailure { original, cleanup },
            )),
        }
    }

    /// Retry native retirement on a finite ordinary or lifecycle worker from the same executor.
    /// A fresh lifecycle job can retire a transfer whose original job was cancelled.
    ///
    /// # Errors
    /// Preserves unsettled native handles and the original retained memory claim.
    pub fn retire(&self, context: &JobContext) -> Result<(), OutputTransferError> {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                tracing::warn!(
                    operation = "comfy_output_retirement",
                    "Recovering poisoned transfer custody"
                );
                poisoned.into_inner()
            }
        };
        state.retire(context)
    }

    /// Observe retirement without blocking a native event loop.
    pub fn settled(&self) -> bool {
        match self.state.try_lock() {
            Ok(state) => state.closed,
            Err(TryLockError::WouldBlock) => false,
            Err(TryLockError::Poisoned(state)) => state.into_inner().closed,
        }
    }
}

impl State {
    fn retire(&mut self, context: &JobContext) -> Result<(), OutputTransferError> {
        if self.closed {
            return Ok(());
        }
        if self.owner.is_some_and(|owner| owner != context.owner_id()) {
            return Err(OutputTransferError::invalid("output_retirement_owner"));
        }
        self.http.close().map_err(|cause| OutputTransferError::new("output_http_close", cause))?;
        self.listener
            .close(context)
            .map_err(|cause| OutputTransferError::new("output_peer_close", cause))?;
        self.workspace = None;
        self.claim = None;
        self.closed = true;
        Ok(())
    }
}

impl Drop for State {
    fn drop(&mut self) {
        if self.started && !self.closed {
            if let Some(claim) = self.claim.take() {
                std::mem::forget(claim);
            }
            tracing::error!(
                operation = "comfy_output_owner_loss",
                "Output transfer retains unresolved native custody"
            );
        }
    }
}

/// Preserve the original transfer or retirement cause without disclosing the selected path.
#[derive(Debug)]
pub struct OutputTransferError {
    stage: &'static str,
    cause: Box<dyn Error + Send + Sync>,
}
impl OutputTransferError {
    pub(super) fn new(stage: &'static str, cause: impl Error + Send + Sync + 'static) -> Self {
        Self { stage, cause: Box::new(cause) }
    }
    pub(super) fn invalid(stage: &'static str) -> Self {
        Self::new(
            stage,
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Comfy output rejected"),
        )
    }
}
impl fmt::Display for OutputTransferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Comfy output transfer failed at {}", self.stage)
    }
}
impl Error for OutputTransferError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

#[derive(Debug)]
struct CombinedFailure {
    original: OutputTransferError,
    cleanup: OutputTransferError,
}
impl fmt::Display for CombinedFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}; retirement also failed: {}", self.original, self.cleanup)
    }
}
impl Error for CombinedFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.original)
    }
}
