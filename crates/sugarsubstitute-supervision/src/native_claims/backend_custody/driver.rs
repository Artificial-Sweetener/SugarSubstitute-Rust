// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serve bounded authenticated custody commands on the existing native worker.
#[path = "driver_failure.rs"]
mod failure;
use super::source::BackendCustodySource;
use crate::ClaimError;
use std::num::NonZeroUsize;
use sugarsubstitute_contracts::{
    backend_custody::{BACKEND_CUSTODY_FRAME_BYTES, BackendCustodyCommand, BackendCustodyFrame},
    execution::{JobContext, JobOutcome, Moment},
};
use sugarsubstitute_execution::{
    backend_custody::{
        BackendCustodyDriver, BackendCustodyMemory, BackendCustodyPipeObservation as Fact,
        BackendCustodyPipeSession, BackendCustodyProgress, BackendCustodyWake,
        LifecyclePipeOperationId,
    },
    windows::NativeObservationError,
};

/// The source is already funded and every driver output precedes its outside alias.
pub(super) struct Driver {
    session: Option<BackendCustodyPipeSession>,
    wake: Option<BackendCustodyWake>,
    read: Option<LifecyclePipeOperationId>,
    write: Option<(LifecyclePipeOperationId, u64)>,
    latest: u64,
    sent: Option<u64>,
    source: BackendCustodySource,
}
impl BackendCustodySource {
    /// Transfer this original session only after checking its genuine source allocation.
    /// Rejection leaves the complete session in `session` without native effects.
    /// # Errors
    /// Rejects an absent or foreign session before allocating a driver.
    pub fn driver(
        &self,
        session: &mut Option<BackendCustodyPipeSession>,
    ) -> Result<Box<dyn BackendCustodyDriver>, BackendCustodyDriverInputError> {
        let original = session.as_ref().ok_or(BackendCustodyDriverInputError::Missing)?;
        if !self.memory().matches_session(original.control()) {
            return Err(BackendCustodyDriverInputError::Foreign);
        }
        let latest = original.control().last_operation();
        let original = session.take().ok_or(BackendCustodyDriverInputError::Missing)?;
        Ok(Box::new(Driver {
            session: Some(original),
            wake: None,
            read: None,
            write: None,
            latest,
            sent: None,
            source: self.clone(),
        }))
    }
}
impl Driver {
    /// Latch an otherwise unrecorded callback error before the worker observes it.
    fn retained<T>(
        &self,
        result: Result<T, NativeObservationError>,
    ) -> Result<T, NativeObservationError> {
        if result.is_err()
            && let Ok(mut access) = self.source.access()
            && let Ok(state) = access.state()
        {
            return failure::retain(result, &mut state.first, &mut state.failed);
        }
        result
    }
    /// Preserve the genuine first cause in durable source state before propagation.
    fn native(&mut self, stage: &'static str, cause: ClaimError) -> NativeObservationError {
        if let Ok(mut access) = self.source.access()
            && let Ok(state) = access.state()
        {
            state.fail(cause.clone());
        }
        NativeObservationError::new(stage, cause)
    }
    /// Advance one connection-local operation without reuse or wrap.
    fn operation(&mut self) -> Result<LifecyclePipeOperationId, NativeObservationError> {
        self.latest = self.latest.checked_add(1).ok_or_else(|| {
            NativeObservationError::new(
                "custody_operation_exhausted",
                std::io::Error::other("Custody operation exhausted"),
            )
        })?;
        LifecyclePipeOperationId::new(self.latest)
            .map_err(|cause| NativeObservationError::new("custody_operation", cause))
    }
    /// Drain at most32 actual facts; all frame loans return before another receive.
    fn observe(&mut self, context: &JobContext) -> Result<(), NativeObservationError> {
        let Some(session) = &mut self.session else {
            return Ok(());
        };
        let facts = session.drain(NonZeroUsize::new(32).ok_or_else(|| {
            NativeObservationError::new(
                "custody_drain_bound",
                std::io::Error::other("Custody drain bound"),
            )
        })?);
        for fact in facts {
            match fact {
                Fact::Received { operation, frame } => {
                    if self.read != Some(operation) {
                        return Err(self.native(
                            "custody_read_correlation",
                            ClaimError::invalid("custody_read_correlation"),
                        ));
                    }
                    self.read = None;
                    self.sent = None;
                    let decoded =
                        BackendCustodyFrame::decode(frame.as_bytes()).map_err(|cause| {
                            self.native("custody_frame", ClaimError::new("custody_frame", cause))
                        })?;
                    let wake = self
                        .wake
                        .as_ref()
                        .ok_or_else(|| {
                            NativeObservationError::new(
                                "custody_driver_wake",
                                std::io::Error::other("Custody driver wake missing"),
                            )
                        })?
                        .clone();
                    let applied = {
                        let mut access = self.source.access().map_err(|cause| {
                            NativeObservationError::new("custody_source_access", cause)
                        })?;
                        access.state().and_then(|state| {
                            state.apply(decoded, self.source.memory(), &wake, context)
                        })
                    };
                    applied.map_err(|cause| self.native("custody_command", cause))?;
                    drop(frame);
                }
                Fact::Written { operation, bytes } => {
                    let Some((expected, nonce)) = self.write.take() else {
                        return Err(self.native(
                            "custody_write_correlation",
                            ClaimError::invalid("custody_write_correlation"),
                        ));
                    };
                    if operation != expected || bytes as usize != BACKEND_CUSTODY_FRAME_BYTES {
                        return Err(self.native(
                            "custody_write_shape",
                            ClaimError::invalid("custody_write_shape"),
                        ));
                    }
                    self.sent = Some(nonce);
                    let mut access = self.source.access().map_err(|cause| {
                        NativeObservationError::new("custody_receipt_owner", cause)
                    })?;
                    access.state().and_then(|state| state.receipt_written(nonce)).map_err(
                        |cause| NativeObservationError::new("custody_receipt_owner", cause),
                    )?;
                }
                Fact::ReadPending { operation } if self.read == Some(operation) => {}
                Fact::WriteIssued { operation, .. }
                    if self.write.is_some_and(|(expected, _)| expected == operation) => {}
                Fact::WriteRevoked { buffer, .. } => {
                    drop(buffer);
                    return Err(self.native(
                        "custody_write_revoked",
                        ClaimError::invalid("custody_write_revoked"),
                    ));
                }
                Fact::Prepared
                | Fact::Connected
                | Fact::Authenticated(_)
                | Fact::ReadPending { .. }
                | Fact::WriteIssued { .. } => {
                    return Err(self.native(
                        "custody_unexpected_observation",
                        ClaimError::invalid("custody_unexpected_observation"),
                    ));
                }
            }
        }
        Ok(())
    }
    /// Queue one fixed receipt and receive using the same original live session.
    fn exchange(&mut self, now: Moment) -> Result<(), NativeObservationError> {
        let receipt = {
            let mut access = self
                .source
                .access()
                .map_err(|cause| NativeObservationError::new("custody_source_access", cause))?;
            let state = access
                .state()
                .map_err(|cause| NativeObservationError::new("custody_source_access", cause))?;
            state.latest.zip(state.reply).filter(|(frame, _)| self.sent != Some(frame.nonce))
        };
        if self.write.is_none()
            && let Some((frame, receipt)) = receipt
        {
            let operation = self.operation()?;
            let session = self.session.as_ref().ok_or_else(|| {
                NativeObservationError::new(
                    "custody_session",
                    std::io::Error::other("Custody session missing"),
                )
            })?;
            if let Some(mut buffer) = session
                .control()
                .take_write_buffer()
                .map_err(|cause| NativeObservationError::new("custody_reply_buffer", cause))?
            {
                let encoded = BackendCustodyFrame {
                    key: frame.key,
                    nonce: frame.nonce,
                    command: BackendCustodyCommand::Receipt(receipt),
                }
                .encode()
                .map_err(|cause| NativeObservationError::new("custody_receipt", cause))?;
                let destination = buffer
                    .as_mut_bytes()
                    .get_mut(..BACKEND_CUSTODY_FRAME_BYTES)
                    .ok_or_else(|| {
                        NativeObservationError::new(
                            "custody_reply_capacity",
                            std::io::Error::other("Custody receipt capacity"),
                        )
                    })?;
                destination.copy_from_slice(&encoded);
                let bound = {
                    let mut access = self.source.access().map_err(|cause| {
                        NativeObservationError::new("custody_reply_owner", cause)
                    })?;
                    access
                        .state()
                        .map_err(|cause| NativeObservationError::new("custody_reply_owner", cause))?
                        .reply_until
                }
                .unwrap_or(session.control().original_deadline());
                let until = now
                    .checked_add(std::time::Duration::from_secs(2))
                    .map_err(|cause| NativeObservationError::new("custody_reply_deadline", cause))?
                    .min(bound);
                self.write = Some((operation, frame.nonce));
                session
                    .control()
                    .write(operation, until, buffer, BACKEND_CUSTODY_FRAME_BYTES)
                    .map_err(|rejected| {
                        NativeObservationError::new("custody_reply", rejected.error)
                    })?;
            }
        }
        // One pending transaction at a time; a Stop retains its original nonce
        // until complete actual settlement has produced and written Released.
        if self.read.is_none() && self.write.is_none() && receipt.is_none() {
            let pending = {
                let mut access = self
                    .source
                    .access()
                    .map_err(|cause| NativeObservationError::new("custody_source_access", cause))?;
                let state = access
                    .state()
                    .map_err(|cause| NativeObservationError::new("custody_source_access", cause))?;
                state.stopping || (state.latest.is_some() && state.reply.is_none())
            };
            if !pending {
                let operation = self.operation()?;
                self.read = Some(operation);
                self.session
                    .as_ref()
                    .ok_or_else(|| {
                        NativeObservationError::new(
                            "custody_session",
                            std::io::Error::other("Custody session missing"),
                        )
                    })?
                    .control()
                    .read_passive(operation)
                    .map_err(|cause| NativeObservationError::new("custody_receive", cause))?;
            }
        }
        Ok(())
    }
}
impl BackendCustodyDriver for Driver {
    fn initialize(
        &mut self,
        _context: &JobContext,
        metadata: &BackendCustodyMemory,
        wake: BackendCustodyWake,
    ) -> Result<(), NativeObservationError> {
        let initialize =
            || {
                if !metadata.same_allocation(self.source.memory()) {
                    return Err(self.native(
                        "custody_driver_original",
                        ClaimError::invalid("custody_driver_original"),
                    ));
                }
                self.wake = Some(wake);
                let result = {
                    let mut access = self.source.access().map_err(|cause| {
                        NativeObservationError::new("custody_source_access", cause)
                    })?;
                    access.state().and_then(|state| state.initialize(metadata))
                };
                result.map_err(|cause| self.native("custody_initialize", cause))?;
                self.exchange(metadata.check_registration().map_err(|cause| {
                    NativeObservationError::new("custody_initialize_clock", cause)
                })?)
            };
        let result = initialize();
        self.retained(result)
    }
    fn progress(
        &mut self,
        context: &JobContext,
        now: Moment,
    ) -> Result<BackendCustodyProgress, NativeObservationError> {
        let mut progress = || {
            self.observe(context)?;
            let result = {
                let wake = self.wake.as_ref().ok_or_else(|| {
                    NativeObservationError::new(
                        "custody_driver_wake",
                        std::io::Error::other("Custody driver wake missing"),
                    )
                })?;
                let mut access = self
                    .source
                    .access()
                    .map_err(|cause| NativeObservationError::new("custody_source_access", cause))?;
                access.state().and_then(|state| state.progress(context, self.source.memory(), wake))
            };
            let settled = result.map_err(|cause| self.native("custody_progress", cause))?;
            self.exchange(now)?;
            if settled {
                let mut access = self.source.access().map_err(|cause| {
                    NativeObservationError::new("custody_completion_owner", cause)
                })?;
                let state = access.state().map_err(|cause| {
                    NativeObservationError::new("custody_completion_owner", cause)
                })?;
                if !state.latest.is_some_and(|frame| {
                    matches!(frame.command, BackendCustodyCommand::Finish { .. })
                        && self.sent != Some(frame.nonce)
                }) {
                    return Ok(BackendCustodyProgress::Settled);
                }
            }
            Ok(BackendCustodyProgress::Pending)
        };
        let result = progress();
        self.retained(result)
    }
    fn retire(
        &mut self,
        context: &JobContext,
        _now: Moment,
    ) -> Result<bool, NativeObservationError> {
        if let Some(session) = &self.session {
            session.control().close();
        }
        // Return pending observations/frame loans before waiting for actual native retirement.
        if let Some(session) = &mut self.session {
            for fact in session.drain(NonZeroUsize::MAX) {
                drop(fact);
            }
        }
        let result = {
            let wake = self.wake.as_ref().ok_or_else(|| {
                NativeObservationError::new(
                    "custody_driver_wake",
                    std::io::Error::other("Custody driver wake missing"),
                )
            })?;
            let mut access = self
                .source
                .access()
                .map_err(|cause| NativeObservationError::new("custody_source_access", cause))?;
            access.state().and_then(|state| {
                state.stop_lost(Moment::from_nanos(u64::MAX))?;
                state.progress(context, self.source.memory(), wake)
            })
        };
        if !result.map_err(|cause| self.native("custody_retirement", cause))? {
            return Ok(false);
        }
        if let Some(session) = &mut self.session {
            let Some(done) = session.take_terminal() else {
                return Ok(false);
            };
            if let JobOutcome::Failure(cause) = done.outcome {
                let mut access = self.source.access().map_err(|cause| {
                    NativeObservationError::new("custody_source_failure", cause)
                })?;
                access
                    .state()
                    .map_err(|cause| NativeObservationError::new("custody_source_failure", cause))?
                    .fail(ClaimError::new("custody_transport_retirement", cause));
            }
        }
        self.session = None;
        Ok(true)
    }
}

/// Inline rejection preserves the caller's complete original session without allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendCustodyDriverInputError {
    /// No original session was supplied.
    Missing,
    /// The session belongs to another actual source allocation.
    Foreign,
}
impl std::fmt::Display for BackendCustodyDriverInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Backend custody driver input rejected: {self:?}")
    }
}
impl std::error::Error for BackendCustodyDriverInputError {}
