// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Transfer typed terminal outcomes independently of scheduler task handles.

use super::{JobId, JobSpec, MemoryLease, ScopeId};
use std::time::Duration;

/// Classify a terminal job for metadata delivery without exposing its value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalKind {
    /// Typed work completed successfully.
    Success,
    /// Typed work returned its domain error.
    Failure,
    /// Supervised work panicked.
    Panicked,
    /// Cancellation was observed after actual work stopped.
    Cancelled,
    /// The effective deadline elapsed and actual work stopped.
    TimedOut,
    /// Newer admitted work replaced this pending job.
    Coalesced,
}

/// Retain at most 1024 UTF-8 bytes describing a supervised panic.
///
/// Text remains owned diagnostic data and is never automatically logged. The
/// adapter converts native panic payloads into text before constructing this type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PanicCause(String);

impl PanicCause {
    /// Bound owned diagnostic text without splitting a UTF-8 code point.
    pub fn new(text: String) -> Self {
        let mut end = text.len().min(1024);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        // Truncating the original String would retain its unbounded allocation.
        Self(text[..end].to_owned())
    }

    /// Borrow the bounded owned diagnostic text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Observe a typed terminal result after the adapter establishes actual termination.
#[derive(Debug)]
pub enum JobOutcome<T, E> {
    /// Successful typed application result.
    Success(T),
    /// Original typed application failure.
    Failure(E),
    /// Bounded supervised panic cause.
    Panicked(PanicCause),
    /// Work stopped after a cancellation request was observed.
    Cancelled,
    /// Work stopped after its effective deadline elapsed.
    TimedOut,
    /// An admitted newer job replaced this pending job before its factory ran.
    Coalesced {
        /// Accepted job replacing this pending generation.
        replacement: JobId,
    },
}

impl<T, E> JobOutcome<T, E> {
    /// Return terminal metadata without borrowing the typed result or error.
    pub const fn kind(&self) -> TerminalKind {
        match self {
            Self::Success(_) => TerminalKind::Success,
            Self::Failure(_) => TerminalKind::Failure,
            Self::Panicked(_) => TerminalKind::Panicked,
            Self::Cancelled => TerminalKind::Cancelled,
            Self::TimedOut => TerminalKind::TimedOut,
            Self::Coalesced { .. } => TerminalKind::Coalesced,
        }
    }
}

/// Transfer one acknowledged typed terminal outcome and its retained byte charge.
///
/// Taking this value acknowledges terminal-delivery credit. Its memory guard and
/// cloned result guards continue to charge retained bytes until their last drop.
#[derive(Debug)]
#[must_use = "Observe the terminal outcome and retain its lease with result buffers"]
pub struct JobCompletion<T, E> {
    /// Application scope that admitted this job.
    pub scope_id: ScopeId,
    /// Exact application job identity and submitted capacity/deadline specification.
    pub spec: JobSpec,
    /// Elapsed admitted queue duration in the injected monotonic clock.
    pub queue_time: Duration,
    /// Elapsed execution duration through actual termination.
    pub execution_time: Duration,
    /// Observed typed terminal outcome.
    pub outcome: JobOutcome<T, E>,
    /// Shared retained-byte reservation, independent of delivery credit.
    pub memory: MemoryLease,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_text_is_bounded_on_utf8_boundaries_without_changing_short_causes() {
        assert_eq!(PanicCause::new("cause".into()).as_str(), "cause");
        let cause = PanicCause::new(format!("{}🧁rest", "x".repeat(1023)));
        assert_eq!(cause.as_str(), "x".repeat(1023));
        let cause = PanicCause::new("糖".repeat(400));
        assert_eq!(cause.as_str().len(), 1023);
        assert!(cause.as_str().chars().all(|character| character == '糖'));
        let mut oversized_allocation = String::with_capacity(65_536);
        oversized_allocation.push_str("short cause");
        let cause = PanicCause::new(oversized_allocation);
        assert_eq!(cause.as_str(), "short cause");
        assert!(cause.0.capacity() <= 1024);
    }
}
