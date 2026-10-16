// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound incident facts independently of storage, capture, presentation and M1 policy.
//!
//! Construction validates shape and related facts, never authentication, durable
//! publication, native capture completion or restart authority. Codecs belong to
//! the crash adapter. The application consumes M1 exit attribution unchanged.

mod eviction;
mod evidence;
mod helper;
mod identity;
mod report;
mod summary;
mod summary_migration;
mod symbols;
#[cfg(test)]
mod tests;
mod time;

pub use eviction::*;
pub use evidence::*;
pub use helper::*;
pub use identity::*;
pub use report::*;
pub use summary::*;
pub use summary_migration::SummaryMigrationStep;
pub use symbols::*;
pub use time::*;

/// Maximum retained diagnostic payload bytes, including retained staging/quarantine.
pub const MAX_PAYLOAD_BYTES: u64 = 256 * 1024 * 1024;
/// Maximum incidents retaining full payloads.
pub const MAX_FULL_INCIDENTS: usize = 32;
/// Maximum retained summaries, including pending and acknowledged records.
pub const MAX_SUMMARIES: usize = 128;
/// Maximum serialized UTF-8 bytes of one summary; the codec checks its encoding.
pub const MAX_SUMMARY_BYTES: usize = 64 * 1024;
/// Maximum text attachment bytes, including any truncation marker.
pub const MAX_TEXT_ATTACHMENT_BYTES: u64 = 256 * 1024;
/// Maximum allowlisted attachments referenced by one incident.
pub const MAX_ATTACHMENTS: usize = 8;
/// Maximum acknowledged incidents retaining full payloads, within the overall cap.
pub const MAX_ACKNOWLEDGED_FULL_INCIDENTS: usize = 20;
/// Acknowledged payload retention interval in UTC seconds: exactly ninety days.
pub const ACKNOWLEDGED_RETENTION_SECONDS: u64 = 90 * 24 * 60 * 60;

/// Reject invalid bounded facts without retaining native errors or user text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncidentError {
    /// Identity text is empty, oversized or has forbidden characters.
    InvalidText,
    /// A positive identity/revision or nonzero dump UUID was required.
    InvalidIdentity,
    /// A revision cannot advance without wrapping.
    RevisionExhausted,
    /// A helper/session operation identity cannot advance without wrapping.
    OperationExhausted,
    /// A cumulative summary-eviction category cannot advance without wrapping.
    EvictionCountExhausted,
    /// UTC time is outside the supported epoch or related timestamps regress.
    InvalidTime,
    /// Subject, revision, build or supplied attribution facts disagree.
    MismatchedFacts,
    /// Byte/count limits, uniqueness or original/retained lengths are invalid.
    InvalidEvidence,
}
impl std::fmt::Display for IncidentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Invalid incident fact: {self:?}")
    }
}
impl std::error::Error for IncidentError {}
