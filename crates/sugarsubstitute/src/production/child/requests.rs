// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlate bounded client replies without granting native application authority.
//!
//! One immutable local/counter deadline pair belongs to each prepared operation.
//! Eight inline slots allow independent application and UI-watchdog requests.
//! Payload validation, output ingestion and actual native effects stay with their
//! semantic owners; a matching reply never establishes readiness or retirement.
use super::handshake::WireHandshake;
#[path = "requests/retirement.rs"]
mod retirement;
pub(super) use retirement::WithdrawnRequest;
#[path = "requests/expiry.rs"]
mod expiry;
pub(super) use expiry::RequestExpiry;
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{
    execution::{CounterDeadline, Moment},
    lifecycle::{ConnectionId, RequestId, RunKey},
};
use sugarsubstitute_supervision::lifecycle_protocol::WireHeader;

/// Maximum outstanding finite application requests, including received replies.
const REQUEST_SLOTS: usize = 8;

/// Reject a correlation without consuming its original identity or deadline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RequestError {
    /// The exact Register acknowledgement has not established wire ordering.
    Registration,
    /// Outbound application admission has closed.
    Closed,
    /// Every fixed slot still belongs to its original request.
    Capacity,
    /// The nonwrapping shared request sequence cannot advance.
    Exhausted,
    /// Full run, connection, request or either original deadline differs.
    Identity,
    /// The encoded frequency differs from the enrolled native clock fact.
    Frequency,
    /// Admission/receipt/semantic consumption occurred in the wrong order.
    Order,
    /// The original local cutoff was reached before send or reply observation.
    Deadline,
}
impl fmt::Display for RequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Production lifecycle request rejected: {self:?}")
    }
}
impl Error for RequestError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Prepared,
    Admitted,
    Received,
    RetiringAdmitted,
    RetiringReceived,
    RetiringUnissued,
}
/// Classify only authored request owners; these facts grant no native effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Purpose {
    Other,
    Acquisition,
    OutputPull,
    BackendRetry,
}
#[derive(Clone, Copy)]
struct Pending {
    header: WireHeader,
    original: Moment,
    phase: Phase,
    purpose: Purpose,
    waits_for_owned_process: bool,
}

/// Retain immutable matching facts separately from native transport/frame custody.
pub(super) struct ClientRequests {
    run: RunKey,
    connection: ConnectionId,
    frequency: i64,
    latest: u64,
    slots: [Option<Pending>; REQUEST_SLOTS],
    closed: bool,
}
impl ClientRequests {
    /// Begin after the actual wire owner accepted its exact Register acknowledgement.
    /// # Errors
    /// Rejects construction before registration; this creates no native authority.
    pub(super) fn new(binding: &WireHandshake) -> Result<Self, RequestError> {
        Ok(Self {
            run: binding.run(),
            connection: binding.connection().ok_or(RequestError::Registration)?,
            frequency: binding.counter_frequency(),
            latest: binding.registered_request().ok_or(RequestError::Registration)?.get(),
            slots: [None; REQUEST_SLOTS],
            closed: false,
        })
    }

    /// Retain one pair encoded once by the composing executor's actual clock owner.
    /// Busy transport retries borrow this same header and local cutoff unchanged.
    /// # Errors
    /// Closed/capacity/frequency/exhaustion errors leave the complete ledger unchanged.
    pub(super) fn prepare(
        &mut self,
        original: Moment,
        encoded: CounterDeadline,
    ) -> Result<WireHeader, RequestError> {
        if self.closed {
            return Err(RequestError::Closed);
        }
        if encoded.frequency() != self.frequency {
            return Err(RequestError::Frequency);
        }
        let slot =
            self.slots.iter_mut().find(|slot| slot.is_none()).ok_or(RequestError::Capacity)?;
        let latest = self.latest.checked_add(1).ok_or(RequestError::Exhausted)?;
        let request = RequestId::new(latest).map_err(|_| RequestError::Exhausted)?;
        let header = WireHeader {
            run: self.run,
            connection: Some(self.connection),
            request: Some(request),
            deadline_qpc: encoded.ticks(),
        };
        *slot = Some(Pending {
            header,
            original,
            phase: Phase::Prepared,
            purpose: Purpose::Other,
            waits_for_owned_process: false,
        });
        self.latest = latest;
        Ok(header)
    }

    /// Check the original pair before attempting actual finite native write admission.
    /// # Errors
    /// Rejects closure, changed pair, duplicate admission or original deadline expiry.
    pub(super) fn outbound(
        &self,
        header: WireHeader,
        original: Moment,
        now: Moment,
    ) -> Result<(), RequestError> {
        if self.closed {
            return Err(RequestError::Closed);
        }
        let pending = self.find(header)?;
        if pending.original != original {
            return Err(RequestError::Identity);
        }
        if pending.phase != Phase::Prepared {
            return Err(RequestError::Order);
        }
        if now >= original {
            return Err(RequestError::Deadline);
        }
        Ok(())
    }

    /// Record only a successful actual queue admission, without sampling another clock.
    /// # Errors
    /// Rejects a changed header or repeated admission without altering prior evidence.
    pub(super) fn admitted(&mut self, header: WireHeader) -> Result<(), RequestError> {
        let pending = self.find_mut(header)?;
        if pending.phase != Phase::Prepared {
            return Err(RequestError::Order);
        }
        pending.phase = Phase::Admitted;
        Ok(())
    }

    /// Record an exact received Reply while retaining its slot through semantic handling.
    /// # Errors
    /// Rejects unsolicited, changed, duplicate, unadmitted or expired replies unchanged.
    pub(super) fn received(&mut self, header: WireHeader, now: Moment) -> Result<(), RequestError> {
        let pending = self.find_mut(header)?;
        if pending.phase != Phase::Admitted {
            return Err(RequestError::Order);
        }
        if now >= pending.original && !pending.waits_for_owned_process {
            return Err(RequestError::Deadline);
        }
        pending.phase = Phase::Received;
        Ok(())
    }

    /// Route an exact authenticated loan only to its already fenced retirement owner.
    /// Ordinary replies keep every original admission/order/cutoff check unchanged.
    pub(super) fn received_or_retiring(
        &mut self,
        header: WireHeader,
        now: Moment,
    ) -> Result<(), RequestError> {
        if self.retiring(header)? {
            self.received_retiring(header)
        } else {
            self.received(header, now)
        }
    }
    /// Bind the configured target policy before finite native admission; no readiness is inferred.
    pub(super) fn acquisition(
        &mut self,
        header: WireHeader,
        kind: sugarsubstitute_application::startup::ConnectionBudgetKind,
    ) -> Result<(), RequestError> {
        let pending = self.find_mut(header)?;
        if pending.phase != Phase::Prepared || pending.purpose != Purpose::Other {
            return Err(RequestError::Order);
        }
        pending.purpose = Purpose::Acquisition;
        pending.waits_for_owned_process = kind.waits_for_owned_process();
        Ok(())
    }
    /// Consume one validated nonfinal frame, retaining its original slot and cutoff.
    /// This cannot reopen a final reply, foreign request, expired budget.
    pub(super) fn progress(&mut self, header: WireHeader, now: Moment) -> Result<(), RequestError> {
        let pending = self.find_mut(header)?;
        if pending.purpose != Purpose::Acquisition || pending.phase != Phase::Received {
            return Err(RequestError::Order);
        }
        if now >= pending.original && !pending.waits_for_owned_process {
            return Err(RequestError::Deadline);
        }
        pending.phase = Phase::Admitted;
        Ok(())
    }
    /// Release correlation metadata after the semantic owner handles the original frame.
    /// This releases no native frame/lease or job and grants no settlement fact.
    /// # Errors
    /// Rejects foreign/unreceived/repeated consumption without releasing another request.
    pub(super) fn consumed(&mut self, header: WireHeader) -> Result<(), RequestError> {
        self.find(header)?;
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.as_ref().is_some_and(|pending| pending.header == header))
            .ok_or(RequestError::Identity)?;
        if !slot.as_ref().is_some_and(|pending| pending.phase == Phase::Received) {
            return Err(RequestError::Order);
        }
        *slot = None;
        Ok(())
    }

    /// Withdraw only a never-admitted original request, preserving identity high-water.
    /// This proves no native retirement and cannot discard an admitted or received reply.
    pub(super) fn withdraw_prepared(&mut self, header: WireHeader) -> Result<(), RequestError> {
        if self.find(header)?.phase != Phase::Prepared {
            return Err(RequestError::Order);
        }
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.as_ref().is_some_and(|pending| pending.header == header))
            .ok_or(RequestError::Identity)?;
        *slot = None;
        Ok(())
    }
    /// Fence new preparation/send without erasing original pending reply evidence.
    pub(super) fn close(&mut self) {
        self.closed = true;
    }

    fn find(&self, header: WireHeader) -> Result<&Pending, RequestError> {
        self.slots
            .iter()
            .flatten()
            .find(|pending| pending.header == header)
            .ok_or(RequestError::Identity)
    }
    fn find_mut(&mut self, header: WireHeader) -> Result<&mut Pending, RequestError> {
        self.slots
            .iter_mut()
            .flatten()
            .find(|pending| pending.header == header)
            .ok_or(RequestError::Identity)
    }
}

#[cfg(test)]
#[path = "request_tests.rs"]
pub(super) mod tests;
