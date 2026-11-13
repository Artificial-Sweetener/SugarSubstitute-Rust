// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow one bounded raw output delivery without claiming transcript ingestion or EOF settlement.
use super::{LeaseReference, ProtocolError};
use std::{fmt, num::NonZeroU64};
use sugarsubstitute_contracts::transcript::{
    MAX_BATCH_BYTES, OutputGap, OutputStream, StreamSequence,
};

/// Borrow the exact raw batch shape; native loan and consume acknowledgment stay with the host.
#[derive(Clone, Copy, PartialEq)]
pub struct OutputDelivery<'a> {
    /// Connection-bound managed lease matching data, not output-capture authority.
    pub lease: LeaseReference,
    /// Positive delivery identity, separate from per-stream producer sequence.
    pub delivery_id: NonZeroU64,
    /// Independently decoded producer stream.
    pub stream: OutputStream,
    /// Exact positive producer sequence within this stream.
    pub sequence: StreamSequence,
    /// Actual preceding raw loss, preserved without text decoding.
    pub gap: Option<OutputGap>,
    /// Actual producer EOF claim; does not prove process, family or worker settlement.
    pub end: bool,
    /// Exact raw bytes, at most32KiB; validation never normalizes invalid UTF8.
    pub bytes: &'a [u8],
}
impl OutputDelivery<'_> {
    pub(super) fn validate(self) -> Result<(), ProtocolError> {
        if self.bytes.len() > MAX_BATCH_BYTES {
            return Err(ProtocolError::OutputShape);
        }
        Ok(())
    }
}
impl fmt::Debug for OutputDelivery<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutputDelivery")
            .field("lease", &self.lease)
            .field("delivery_id", &self.delivery_id)
            .field("stream", &self.stream)
            .field("sequence", &self.sequence)
            .field("gap", &self.gap)
            .field("end", &self.end)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}
