// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind startup wire ordering to one run and original counter deadline.
//!
//! These checks consume protocol facts. Actual peer authentication and retained
//! native custody belong to the lifecycle transport, not to this state machine.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{
    configuration::BackendDataDisposition,
    execution::CounterDeadline,
    lifecycle::{ConnectionId, RequestId, RunKey},
};
use sugarsubstitute_supervision::lifecycle_protocol::{
    DecodedFrame, DecodedPayload, ReplyOutcome, WireHeader,
};

/// Reject a startup exchange without advancing its original wire binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HandshakeError {
    /// An exchange was duplicated or arrived before its prerequisite admission.
    Order,
    /// Full run, connection, request or original deadline differs.
    Identity,
    /// The enrolled counter frequency differs from the actual Welcome.
    Frequency,
    /// An unexpected payload or unsuccessful Register disposition arrived.
    Payload,
}
impl fmt::Display for HandshakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Production lifecycle handshake rejected: {self:?}")
    }
}
impl Error for HandshakeError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Initial,
    HelloAdmitted,
    Welcome(ConnectionId),
    RegisterAdmitted { connection: ConnectionId, request: RequestId },
    Registered { connection: ConnectionId, request: RequestId, disposition: BackendDataDisposition },
}

/// Retain only immutable wire facts; no constructor establishes native readiness.
pub(super) struct WireHandshake {
    run: RunKey,
    deadline: CounterDeadline,
    phase: Phase,
}
impl WireHandshake {
    /// Bind the transport-prepared run and unchanged enrolled counter facts.
    pub(super) const fn new(run: RunKey, deadline: CounterDeadline) -> Self {
        Self { run, deadline, phase: Phase::Initial }
    }

    /// Describe the original Hello before its actual native write admission.
    pub(super) fn hello_header(&self) -> Result<WireHeader, HandshakeError> {
        if self.phase != Phase::Initial {
            return Err(HandshakeError::Order);
        }
        Ok(self.header(None, None))
    }

    /// Record actual queue admission, independently of later native write completion.
    /// A server can answer before the client's Written observation is drained.
    pub(super) fn hello_admitted(&mut self) -> Result<(), HandshakeError> {
        self.hello_header()?;
        self.phase = Phase::HelloAdmitted;
        Ok(())
    }

    /// Bind an exact Welcome without accepting an unsolicited or extended exchange.
    /// Errors leave every prior binding intact.
    pub(super) fn welcome(&mut self, frame: &DecodedFrame<'_>) -> Result<(), HandshakeError> {
        if self.phase != Phase::HelloAdmitted {
            return Err(HandshakeError::Order);
        }
        let connection = frame.header.connection.ok_or(HandshakeError::Identity)?;
        if frame.header != self.header(Some(connection), None) {
            return Err(HandshakeError::Identity);
        }
        match frame.payload {
            DecodedPayload::Welcome { qpc_frequency } => {
                if qpc_frequency != self.deadline.frequency() {
                    return Err(HandshakeError::Frequency);
                }
            }
            _ => return Err(HandshakeError::Payload),
        }
        self.phase = Phase::Welcome(connection);
        Ok(())
    }

    /// Describe one Register request using the coordinator-assigned connection.
    pub(super) fn register_header(&self, request: RequestId) -> Result<WireHeader, HandshakeError> {
        let Phase::Welcome(connection) = self.phase else { return Err(HandshakeError::Order) };
        Ok(self.header(Some(connection), Some(request)))
    }

    /// Record actual admission of the exact Register, retaining its request identity.
    pub(super) fn register_admitted(&mut self, request: RequestId) -> Result<(), HandshakeError> {
        self.register_header(request)?;
        let Phase::Welcome(connection) = self.phase else { return Err(HandshakeError::Order) };
        self.phase = Phase::RegisterAdmitted { connection, request };
        Ok(())
    }

    /// Accept only the exact acknowledged Register response.
    /// This establishes wire ordering, never backend or browser readiness.
    pub(super) fn registered(&mut self, frame: &DecodedFrame<'_>) -> Result<(), HandshakeError> {
        let Phase::RegisterAdmitted { connection, request } = self.phase else {
            return Err(HandshakeError::Order);
        };
        if frame.header != self.header(Some(connection), Some(request)) {
            return Err(HandshakeError::Identity);
        }
        let DecodedPayload::Reply(ReplyOutcome::Registered { backend_data_disposition }) =
            frame.payload
        else {
            return Err(HandshakeError::Payload);
        };
        self.phase =
            Phase::Registered { connection, request, disposition: backend_data_disposition };
        Ok(())
    }

    /// Borrow only the exact policy carried by the matching Register response. Authentication
    /// remains with the composing native channel; there is no pre-registration fallback.
    pub(super) const fn backend_data_disposition(&self) -> Option<BackendDataDisposition> {
        match self.phase {
            Phase::Registered { disposition, .. } => Some(disposition),
            _ => None,
        }
    }

    /// Return the assigned connection only after its exact Register acknowledgement.
    pub(super) const fn connection(&self) -> Option<ConnectionId> {
        match self.phase {
            Phase::Registered { connection, .. } => Some(connection),
            _ => None,
        }
    }

    /// Return the full original run for additional request ownership checks.
    pub(super) const fn run(&self) -> RunKey {
        self.run
    }

    /// Return the acknowledged Register identity for the sole outbound sequence.
    pub(super) const fn registered_request(&self) -> Option<RequestId> {
        match self.phase {
            Phase::Registered { request, .. } => Some(request),
            _ => None,
        }
    }

    /// Return the unchanged enrolled ticks-per-second fact for later requests.
    pub(super) const fn counter_frequency(&self) -> i64 {
        self.deadline.frequency()
    }

    fn header(&self, connection: Option<ConnectionId>, request: Option<RequestId>) -> WireHeader {
        WireHeader { run: self.run, connection, request, deadline_qpc: self.deadline.ticks() }
    }
}

#[cfg(test)]
#[path = "handshake_tests.rs"]
mod tests;
