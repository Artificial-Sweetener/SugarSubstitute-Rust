// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlate native observations with their original admitted transport operations.
//!
//! These records contain matching facts only. Their construction establishes no
//! admission, peer authentication, frame ownership or native retirement.
use std::{error::Error, fmt};
use sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeOperationId;

/// Reject an observation while preserving the original pending operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ObservationError {
    /// No matching original read admission remains.
    ReadIdentity,
    /// A write observation differs from its original admission.
    WriteIdentity,
    /// Issuance was repeated or completion preceded actual issuance.
    WriteOrder,
    /// The reported complete write size differs from the encoded frame.
    WriteBytes,
}
impl fmt::Display for ObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Production lifecycle observation rejected: {self:?}")
    }
}
impl Error for ObservationError {}

/// Retain an actual admitted read identity until its received frame transfers.
pub(super) struct PendingRead(LifecyclePipeOperationId);
impl PendingRead {
    /// Record the operation after the actual transport accepts its read command.
    pub(super) const fn new(operation: LifecyclePipeOperationId) -> Self {
        Self(operation)
    }

    /// Check pending/received observations without consuming the original record.
    pub(super) fn validate(
        pending: Option<&Self>,
        operation: LifecyclePipeOperationId,
    ) -> Result<(), ObservationError> {
        if pending.is_some_and(|pending| pending.0 == operation) {
            Ok(())
        } else {
            Err(ObservationError::ReadIdentity)
        }
    }
}

/// Retain the exact encoded byte count through native issuance and completion.
pub(super) struct PendingWrite {
    operation: LifecyclePipeOperationId,
    bytes: usize,
    issued: bool,
}
impl PendingWrite {
    /// Record the operation only after its existing native write queue accepts it.
    pub(super) const fn new(operation: LifecyclePipeOperationId, bytes: usize) -> Self {
        Self { operation, bytes, issued: false }
    }

    /// Record one actual issuance; errors preserve the previous original state.
    pub(super) fn issued(
        pending: Option<&mut Self>,
        operation: LifecyclePipeOperationId,
    ) -> Result<(), ObservationError> {
        let pending = pending.ok_or(ObservationError::WriteIdentity)?;
        if pending.operation != operation {
            return Err(ObservationError::WriteIdentity);
        }
        if pending.issued {
            return Err(ObservationError::WriteOrder);
        }
        pending.issued = true;
        Ok(())
    }

    /// Validate completion before the caller releases its original admission record.
    /// Native completion is independent of server replies and application effects.
    pub(super) fn written(
        pending: Option<&Self>,
        operation: LifecyclePipeOperationId,
        bytes: u32,
    ) -> Result<(), ObservationError> {
        let pending = pending.ok_or(ObservationError::WriteIdentity)?;
        if pending.operation != operation {
            return Err(ObservationError::WriteIdentity);
        }
        if !pending.issued {
            return Err(ObservationError::WriteOrder);
        }
        if usize::try_from(bytes).ok() != Some(pending.bytes) {
            return Err(ObservationError::WriteBytes);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "observation_tests.rs"]
mod tests;
