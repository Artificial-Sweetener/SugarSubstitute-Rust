// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate bounded stream facts before copying input bytes.
use super::{MAX_BATCH_BYTES, TranscriptError};
use crate::lifecycle::ResourceKey;
use std::{fmt, num::NonZeroU64};

/// Identify independently decoded child output streams.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputStream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}
impl OutputStream {
    /// Return the fixed array index, stdout zero and stderr one.
    pub const fn index(self) -> usize {
        match self {
            Self::Stdout => 0,
            Self::Stderr => 1,
        }
    }
}
/// Positive per-stream input sequence, distinct from record identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StreamSequence(NonZeroU64);
impl StreamSequence {
    /// Reject zero without allocating.
    pub const fn new(value: u64) -> Result<Self, TranscriptError> {
        match NonZeroU64::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(TranscriptError::ZeroIdentity),
        }
    }
    /// Return the producer's exact sequence.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}
/// Report a discontinuity before this batch; it resets only this stream's parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputGap {
    dropped_bytes: u64,
    skipped_batches: u64,
}
impl OutputGap {
    /// Reject zero dropped bytes; skipped batches may be zero for a partial-batch loss.
    pub const fn new(dropped_bytes: u64, skipped_batches: u64) -> Result<Self, TranscriptError> {
        if dropped_bytes == 0 {
            Err(TranscriptError::EmptyGap)
        } else {
            Ok(Self { dropped_bytes, skipped_batches })
        }
    }
    /// Return missing raw bytes, never a guessed decoded-text length.
    pub const fn dropped_bytes(self) -> u64 {
        self.dropped_bytes
    }
    /// Return missing complete sequence positions.
    pub const fn skipped_batches(self) -> u64 {
        self.skipped_batches
    }
}
/// Own one bounded byte batch; construction establishes shape, not producer trust.
pub struct OutputBatch {
    resource: ResourceKey,
    stream: OutputStream,
    sequence: StreamSequence,
    bytes: Box<[u8]>,
    gap: Option<OutputGap>,
    end: bool,
}
impl fmt::Debug for OutputBatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutputBatch")
            .field("resource", &self.resource)
            .field("stream", &self.stream)
            .field("sequence", &self.sequence)
            .field("byte_count", &self.bytes.len())
            .field("gap", &self.gap)
            .field("end", &self.end)
            .finish()
    }
}
impl OutputBatch {
    /// Check the 32KiB bound before copying; `end` marks actual producer EOF.
    /// Empty batches can carry a gap or EOF. EOF is not process/family settlement.
    pub fn new(
        resource: ResourceKey,
        stream: OutputStream,
        sequence: StreamSequence,
        bytes: &[u8],
        gap: Option<OutputGap>,
        end: bool,
    ) -> Result<Self, TranscriptError> {
        if bytes.len() > MAX_BATCH_BYTES {
            return Err(TranscriptError::Bound);
        }
        Ok(Self { resource, stream, sequence, bytes: bytes.into(), gap, end })
    }
    /// Return exact resource generation matching facts.
    pub const fn resource(&self) -> ResourceKey {
        self.resource
    }
    /// Return the independently decoded stream.
    pub const fn stream(&self) -> OutputStream {
        self.stream
    }
    /// Return the exact per-stream sequence.
    pub const fn sequence(&self) -> StreamSequence {
        self.sequence
    }
    /// Borrow bounded raw foreign bytes without formatting them.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Return explicit preceding loss.
    pub const fn gap(&self) -> Option<OutputGap> {
        self.gap
    }
    /// Return whether no further batches may enter this stream.
    pub const fn end(&self) -> bool {
        self.end
    }
}
