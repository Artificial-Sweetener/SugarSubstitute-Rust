// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep one original server challenge through actual UI dispatch and its response.
use super::{super::requests::ClientRequests, WireHandshake};
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{
    execution::{CounterDeadline, Moment},
    lifecycle::{ConnectionId, RunKey, UiChallengeId, UiChallengeKey},
};
use sugarsubstitute_supervision::lifecycle_protocol::{ReplyOutcome, WireHeader};

/// Reject stale matching data without changing the original pending exchange.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum ChallengeError {
    /// No exact Register acknowledgement exists for this exchange.
    Registration,
    /// The channel has fenced positive UI work.
    Closed,
    /// Full run, connection, request shape or original counter facts differ.
    Identity,
    /// Another original challenge still awaits dispatch, response or acknowledgement.
    Busy,
    /// The server repeated or decreased its per-run challenge identity.
    Replay,
    /// The native callback or transport transition arrived out of order.
    Order,
    /// The unchanged local cutoff was reached.
    Deadline,
    /// The coordinator rejected the exact response.
    Rejected,
}
impl fmt::Display for ChallengeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Production UI challenge rejected: {self:?}")
    }
}
impl Error for ChallengeError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Queued,
    Dispatched,
    Processed,
    Prepared(WireHeader),
    Admitted(WireHeader),
}
struct Pending {
    key: UiChallengeKey,
    counter: CounterDeadline,
    original: Moment,
    phase: Phase,
}

/// Retain immutable response facts for the same finite native admission attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PreparedUiResponse {
    pub(super) header: WireHeader,
    pub(super) original: Moment,
    pub(super) id: UiChallengeId,
}

/// Store inline matching metadata, never native handles or readiness authority.
/// The composing channel supplies the actual decoded cutoff and actual UI callback.
pub(super) struct UiChallengeExchange {
    run: RunKey,
    connection: ConnectionId,
    frequency: i64,
    latest: u64,
    pending: Option<Pending>,
    closed: bool,
}
impl UiChallengeExchange {
    /// Begin only after the real channel accepted its exact Register reply.
    pub(super) fn new(binding: &WireHandshake) -> Result<Self, ChallengeError> {
        Ok(Self {
            run: binding.run(),
            connection: binding.connection().ok_or(ChallengeError::Registration)?,
            frequency: binding.counter_frequency(),
            latest: 0,
            pending: None,
            closed: false,
        })
    }

    /// Retain the original server ticks and the one conservatively decoded local cutoff.
    /// No native callback or response is inferred from receipt of this frame.
    pub(super) fn receive(
        &mut self,
        header: WireHeader,
        id: UiChallengeId,
        original: Moment,
        now: Moment,
    ) -> Result<(), ChallengeError> {
        if self.closed {
            return Err(ChallengeError::Closed);
        }
        if header.run != self.run
            || header.connection != Some(self.connection)
            || header.request.is_some()
        {
            return Err(ChallengeError::Identity);
        }
        let counter = CounterDeadline::new(header.deadline_qpc, self.frequency)
            .map_err(|_| ChallengeError::Identity)?;
        if id.get() <= self.latest {
            return Err(ChallengeError::Replay);
        }
        if self.pending.is_some() {
            return Err(ChallengeError::Busy);
        }
        if now >= original {
            return Err(ChallengeError::Deadline);
        }
        self.pending = Some(Pending {
            key: UiChallengeKey { run: self.run, id },
            counter,
            original,
            phase: Phase::Queued,
        });
        self.latest = id.get();
        Ok(())
    }

    /// Move one queued key to the actual native command drain, at most once.
    /// This records delivery, not actual UI progress or successful transport issue.
    pub(super) fn dispatch(
        &mut self,
        now: Moment,
    ) -> Result<Option<UiChallengeKey>, ChallengeError> {
        if self.closed {
            return Ok(None);
        }
        let Some(pending) = &mut self.pending else { return Ok(None) };
        if pending.phase != Phase::Queued {
            return Ok(None);
        }
        if now >= pending.original {
            return Err(ChallengeError::Deadline);
        }
        pending.phase = Phase::Dispatched;
        Ok(Some(pending.key))
    }

    /// Accept only the callback returned by Desktop's actual native challenge dispatch.
    /// Closing retires queued callbacks without permitting a response.
    pub(super) fn processed(
        &mut self,
        key: UiChallengeKey,
        now: Moment,
    ) -> Result<(), ChallengeError> {
        if self.closed {
            return Ok(());
        }
        let pending = self.pending.as_mut().ok_or(ChallengeError::Order)?;
        if key != pending.key {
            return Err(ChallengeError::Identity);
        }
        if pending.phase != Phase::Dispatched {
            return Err(ChallengeError::Order);
        }
        if now >= pending.original {
            return Err(ChallengeError::Deadline);
        }
        pending.phase = Phase::Processed;
        Ok(())
    }

    /// Prepare through the same bounded client sequence after actual UI processing.
    /// Contention retries preserve the original server ticks, local cutoff and request.
    pub(super) fn response(
        &mut self,
        requests: &mut ClientRequests,
        now: Moment,
    ) -> Result<Option<PreparedUiResponse>, Box<dyn Error>> {
        if self.closed {
            return Ok(None);
        }
        let Some(pending) = &mut self.pending else { return Ok(None) };
        if now >= pending.original {
            return Err(Box::new(ChallengeError::Deadline));
        }
        if pending.phase == Phase::Processed {
            pending.phase = Phase::Prepared(requests.prepare(pending.original, pending.counter)?);
        }
        Ok(match pending.phase {
            Phase::Prepared(header) => {
                Some(PreparedUiResponse { header, original: pending.original, id: pending.key.id })
            }
            _ => None,
        })
    }

    /// Record only successful admission of the unchanged response to the real pipe.
    pub(super) fn admitted(&mut self, header: WireHeader) -> Result<(), ChallengeError> {
        let pending = self.pending.as_mut().ok_or(ChallengeError::Order)?;
        if pending.phase != Phase::Prepared(header) {
            return Err(ChallengeError::Identity);
        }
        pending.phase = Phase::Admitted(header);
        Ok(())
    }

    /// Fence the shared pipe send path before any provider or native admission.
    /// An independently prepared client request cannot stand in for actual UI processing.
    pub(super) fn validate_response(
        &self,
        header: WireHeader,
        original: Moment,
        id: UiChallengeId,
    ) -> Result<(), ChallengeError> {
        if self.closed {
            return Err(ChallengeError::Closed);
        }
        let pending = self.pending.as_ref().ok_or(ChallengeError::Order)?;
        if pending.phase != Phase::Prepared(header)
            || pending.original != original
            || pending.key.id != id
        {
            return Err(ChallengeError::Identity);
        }
        Ok(())
    }

    /// Select only this exchange's exact response reply, leaving other requests independent.
    pub(super) fn matches_reply(&self, header: WireHeader) -> bool {
        self.pending.as_ref().is_some_and(|pending| pending.phase == Phase::Admitted(header))
    }

    /// Consume matching correlation only after its exact successful semantic acknowledgement.
    /// Rejection keeps both the pending exchange and received request slot as evidence.
    pub(super) fn acknowledged(
        &mut self,
        requests: &mut ClientRequests,
        header: WireHeader,
        outcome: ReplyOutcome<'_>,
    ) -> Result<(), Box<dyn Error>> {
        if !self.matches_reply(header) {
            return Err(Box::new(ChallengeError::Identity));
        }
        if !matches!(outcome, ReplyOutcome::Acknowledged) {
            return Err(Box::new(ChallengeError::Rejected));
        }
        requests.consumed(header)?;
        self.pending = None;
        Ok(())
    }

    /// Expose the original wake deadline without sampling a provider or renewing it.
    pub(super) fn deadline(&self) -> Option<Moment> {
        if self.closed { None } else { self.pending.as_ref().map(|pending| pending.original) }
    }

    /// Fence UI dispatch and response while retaining original matching evidence.
    pub(super) fn close(&mut self) {
        self.closed = true;
    }
}

#[cfg(test)]
#[path = "challenge_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "challenge_send_tests.rs"]
mod send_tests;
