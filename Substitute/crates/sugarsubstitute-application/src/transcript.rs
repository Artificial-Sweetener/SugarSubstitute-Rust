// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded styled output independently of native transport, scheduling and painting.
//!
//! Each accepted turn inspects at most32KiB. Two stream decoders/current lines are
//! independent. CR replaces the current progress record when the next non-LF
//! character arrives; CRLF seals it once. This owner performs no terminal cursor
//! emulation. Gaps seal incomplete text and reset only the affected stream.
//!
//! History is bounded by text/line/style counts. Current-line scratch is at most
//! two64KiB strings/two2048-run arrays; ANSI scratch is64 bytes per stream. Four
//! snapshots each pin at most128 records/256KiB text/16384 runs. Private Arc ownership
//! cannot be extracted or cloned from a snapshot. No persisted format is introduced.
mod ansi;
mod parser;
mod snapshot;
mod store;
mod utf8;
pub use snapshot::{MAX_SNAPSHOT_BYTES, MAX_SNAPSHOT_RECORDS, TranscriptSnapshot};
pub use store::TranscriptStore;
pub use sugarsubstitute_contracts::transcript::{TranscriptChange, TranscriptError};
/// Conservative observable allocation bound for history, two current-line buffers,
/// temporary publication and four snapshot-pinned prior views. Excludes allocator
/// implementation overhead and copies deliberately made by external consumers.
pub const MAX_OWNER_ALLOCATION_BYTES: usize = 16 * 1024 * 1024;
#[cfg(test)]
mod tests;
