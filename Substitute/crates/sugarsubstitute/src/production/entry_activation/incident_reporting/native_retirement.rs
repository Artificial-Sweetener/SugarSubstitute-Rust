// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the final authenticated observation pending until its exact router completion is consumed.
use sugarsubstitute_crash::diagnostic_client::{DiagnosticRequestId, DiagnosticWireError};

/// Receiving this message grants no family/pipe retirement or durable acknowledgement.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum NativeRetirement {
    #[default]
    Awaiting,
    Received(DiagnosticRequestId),
    Completed,
}
impl NativeRetirement {
    /// Called only after the original authenticated NativeRetired frame passes binding validation.
    pub(super) fn receive(
        &mut self,
        request: DiagnosticRequestId,
    ) -> Result<(), DiagnosticWireError> {
        if *self != Self::Awaiting {
            return Err(DiagnosticWireError::Stale);
        }
        *self = Self::Received(request);
        Ok(())
    }
    /// Match the actual router result before disposing its original frame and issued token.
    /// Only true suppresses a further passive read; rejection leaves the pending phase unchanged.
    pub(super) fn consume_completion(
        &mut self,
        request: DiagnosticRequestId,
        accepted: bool,
    ) -> Result<bool, DiagnosticWireError> {
        if !accepted {
            return Err(DiagnosticWireError::Stale);
        }
        match *self {
            Self::Awaiting => Ok(false),
            Self::Received(original) if original == request => {
                *self = Self::Completed;
                Ok(true)
            }
            Self::Received(_) | Self::Completed => Err(DiagnosticWireError::Stale),
        }
    }
    /// Natural close may start only after the final completion has actually been consumed.
    pub(super) fn completed(self) -> bool {
        self == Self::Completed
    }
}
