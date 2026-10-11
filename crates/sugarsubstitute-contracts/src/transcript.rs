// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound process-only output facts and immutable styled records without native authority.
mod input;
mod record;
pub use input::{OutputBatch, OutputGap, OutputStream, StreamSequence};
pub use record::{AnsiColor, RecordId, StyleRun, TextStyle, TranscriptRecord};
use std::{error::Error, fmt};

/// Maximum bytes inspected in one parser turn.
pub const MAX_BATCH_BYTES: usize = 32 * 1024;
/// Maximum UTF-8 text retained in one record.
pub const MAX_RECORD_BYTES: usize = 64 * 1024;
/// Maximum retained records, including incomplete lines.
pub const MAX_RECORDS: usize = 2000;
/// Maximum retained UTF-8 text, excluding separately bounded metadata.
pub const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;
/// Maximum style runs in one record.
pub const MAX_RECORD_RUNS: usize = 2048;
/// Maximum style runs across the retained history.
pub const MAX_STYLE_RUNS: usize = 16_384;

/// Reject malformed bounded input without formatting foreign content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptError {
    /// A numeric identity must be positive.
    ZeroIdentity,
    /// A declared input or metadata bound is exceeded.
    Bound,
    /// A gap must report at least one dropped byte.
    EmptyGap,
    /// A style range is invalid or does not cover the text exactly.
    InvalidStyle,
    /// The resource, including owner and generation, differs.
    ForeignResource,
    /// Sequence does not equal the checked successor plus declared skipped batches.
    Sequence,
    /// The stream has already ended.
    StreamEnded,
    /// An identity or revision cannot advance; the rejected input makes no mutation.
    Exhausted,
    /// Four immutable snapshots are already live.
    SnapshotCapacity,
}
impl fmt::Display for TranscriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Transcript input rejected: {self:?}")
    }
}
impl Error for TranscriptError {}

#[cfg(test)]
mod tests;

/// Count observable loss without inserting owned messages into foreign text.
/// Counters saturate rather than wrap; `counter_saturated` makes that fact visible.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TranscriptDiagnostics {
    /// Dropped input bytes, indexed stdout then stderr.
    pub dropped_bytes: [u64; 2],
    /// Explicitly skipped input batches, indexed stdout then stderr.
    pub skipped_batches: [u64; 2],
    /// UTF-8 bytes omitted after a text or style-run record bound.
    pub truncated_bytes: u64,
    /// Records that reached a text or style-run bound.
    pub truncated_records: u64,
    /// Old records evicted by history text, line or style bounds.
    pub trimmed_records: u64,
    /// UTF-8 text bytes removed by those evictions.
    pub trimmed_bytes: u64,
    /// Invalid/incomplete UTF-8 maximal subparts replaced with U+FFFD.
    pub unicode_replacements: u64,
    /// Malformed or overlong escape sequences discarded.
    pub malformed_escapes: u64,
    /// One or more additions overflowed and saturated at u64::MAX.
    pub counter_saturated: bool,
}

/// Describe one accepted turn with constant-size identity metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranscriptChange {
    /// New checked store revision.
    pub revision: u64,
    /// Inclusive newly allocated ID range; some may already have been trimmed.
    pub appended: Option<(RecordId, RecordId)>,
    /// Previously retained records replaced by this turn, stdout then stderr.
    pub replaced: [Option<RecordId>; 2],
    /// Highest ID evicted this turn; selection can test actual retained membership.
    pub trimmed_through: Option<RecordId>,
    /// Cumulative observable loss and decoding facts.
    pub diagnostics: TranscriptDiagnostics,
}
