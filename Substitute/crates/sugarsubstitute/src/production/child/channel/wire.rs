// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check application message direction and original registered wire matching facts.
use super::WireHandshake;
use std::{error::Error, fmt};
use sugarsubstitute_supervision::lifecycle_protocol::{
    DecodedFrame, DecodedPayload, WireMessage, WirePayload,
};

/// Reject direction or matching data without granting native authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ApplicationWireError {
    /// Full run, connection or positive client request facts differ.
    Binding,
    /// The payload belongs to registration or the opposite protocol direction.
    Payload,
}
impl fmt::Display for ApplicationWireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Production lifecycle application frame rejected: {self:?}")
    }
}
impl Error for ApplicationWireError {}

/// Select the existing matching/dispatch owner without manufacturing a trusted event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum IncomingKind {
    /// Exact client reply matching remains with its original fixed request ledger.
    Reply,
    /// Native UI dispatch and deadline provenance remain with the watchdog owner.
    Challenge,
}

/// Validate a client application message before encoding into actual transport storage.
/// # Errors
/// Rejects mismatched binding/shape, registration frames or server-only payloads.
pub(super) fn outbound(
    binding: &WireHandshake,
    message: &WireMessage<'_>,
) -> Result<(), ApplicationWireError> {
    if message.header.run != binding.run()
        || message.header.connection != binding.connection()
        || message.header.connection.is_none()
        || message.header.request.is_none()
        || message.header.deadline_qpc <= 0
    {
        return Err(ApplicationWireError::Binding);
    }
    if matches!(
        message.payload,
        WirePayload::Hello(_)
            | WirePayload::Welcome { .. }
            | WirePayload::Register
            | WirePayload::Reply(_)
            | WirePayload::UiChallenge(_)
    ) {
        return Err(ApplicationWireError::Payload);
    }
    Ok(())
}

/// Classify a complete codec-validated server frame against the original registered run.
/// This borrows matching facts, never actual authenticated/native privileges.
/// # Errors
/// Rejects foreign bindings and unsupported client/registration payloads.
pub(super) fn incoming(
    binding: &WireHandshake,
    frame: &DecodedFrame<'_>,
) -> Result<IncomingKind, ApplicationWireError> {
    if frame.header.run != binding.run() || frame.header.connection != binding.connection() {
        return Err(ApplicationWireError::Binding);
    }
    match frame.payload {
        DecodedPayload::Reply(_) => Ok(IncomingKind::Reply),
        DecodedPayload::UiChallenge(_) => Ok(IncomingKind::Challenge),
        _ => Err(ApplicationWireError::Payload),
    }
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
