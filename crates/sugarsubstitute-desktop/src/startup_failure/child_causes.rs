// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Name the closed categories of the production child's own delivery failures.
//!
//! The child projects its concrete lifecycle-pipe, UI-challenge, output and clock errors into
//! these categories so a failed delivery reports what actually went wrong. Each category is
//! emitted through its `Debug` token and decoded by the parent relay with the failure frame's
//! vocabulary parsers; no category carries message text, paths or native authority.
use sugarsubstitute_contracts::execution::TerminalKind;

/// A lifecycle pipe command rejection, mirroring the execution pipe owner's closed error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupPipeCause {
    /// The session is stopping or no longer current.
    Closed,
    /// The fixed operation, buffer or observation slot is occupied.
    Busy,
    /// Authentication or connection preparation is not established.
    NotReady,
    /// A zero, reused or decreasing operation identity was supplied.
    InvalidOperation,
    /// The original command cutoff had already passed.
    DeadlineExpired,
    /// A command attempted to extend the admitted handshake cutoff.
    InvalidDeadline,
    /// The message is empty or exceeds the fixed bound.
    InvalidMessage,
    /// A single-transfer native artifact was already taken.
    AlreadyTransferred,
}

/// A rejected coordinator UI watchdog challenge exchange.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupChallengeCause {
    /// No registration acknowledgement bound the exchange.
    Registration,
    /// The channel had fenced UI work.
    Closed,
    /// Run, connection, request shape or counter facts differ.
    Identity,
    /// A newer challenge arrived while an earlier one was still pending.
    Busy,
    /// The coordinator repeated or decreased its challenge identity.
    Replay,
    /// A native callback or transport transition arrived out of order.
    Order,
    /// The coordinator rejected the child's response.
    Rejected,
}

/// A managed output ingestion failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupOutputCause {
    /// Finite parser admission failed before the parser ran.
    Input,
    /// The execution clock failed before parser admission.
    Clock,
    /// The parser deadline could not be represented.
    Time,
    /// The parser reservation could not fund its bound.
    Claim,
    /// The coordinator frame failed closed wire decoding.
    Protocol,
    /// Header, lease, delivery or job identity differs.
    Identity,
    /// A pull, job or frame already occupied the sole slot, or admission closed.
    Order,
    /// The parser job was cancelled before its effect.
    Cancelled,
    /// The parser's finite cutoff passed before it finished.
    Deadline,
    /// Retained parser state was poisoned.
    Poisoned,
    /// The parser job ended without a success value.
    Terminal(TerminalKind),
}

/// An invalid monotonic clock observation or unrepresentable deadline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupTimeCause {
    /// A deadline or moment exceeds its representable range.
    Overflow,
    /// A supplied time ran backwards relative to an earlier observation.
    Regression,
}

#[cfg(test)]
#[path = "child_causes_tests.rs"]
mod tests;
