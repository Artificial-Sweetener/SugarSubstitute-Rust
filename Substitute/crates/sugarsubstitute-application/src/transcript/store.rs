// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit exact stream sequences and publish bounded immutable history.
mod retention;
use super::{parser::Parser, snapshot::SnapshotAdmission};
use std::{collections::VecDeque, sync::Arc};
use sugarsubstitute_contracts::{lifecycle::ResourceKey, transcript::*};
#[derive(Default)]
struct Line {
    id: Option<RecordId>,
    text: String,
    runs: Vec<StyleRun>,
    cr: bool,
    truncated: bool,
    published: bool,
}
impl Line {
    fn clear(&mut self) {
        self.text.clear();
        self.runs.clear();
        self.cr = false;
        self.truncated = false;
    }
}
#[derive(Default)]
struct Stream {
    parser: Parser,
    line: Line,
    sequence: u64,
    ended: bool,
}

/// Own one resource generation's stdout/stderr transcript and snapshot admission.
/// This is a single mutable application owner, independent of execution/native I/O.
pub struct TranscriptStore {
    resource: ResourceKey,
    revision: u64,
    next_id: u64,
    streams: [Stream; 2],
    pub(super) records: VecDeque<Arc<TranscriptRecord>>,
    text_bytes: usize,
    style_runs: usize,
    diagnostics: TranscriptDiagnostics,
    pub(super) snapshots: Arc<SnapshotAdmission>,
}
impl TranscriptStore {
    /// Create empty independent parser state without native effects or clocks.
    pub fn new(resource: ResourceKey) -> Self {
        Self {
            resource,
            revision: 0,
            next_id: 1,
            streams: Default::default(),
            records: VecDeque::with_capacity(MAX_RECORDS),
            text_bytes: 0,
            style_runs: 0,
            diagnostics: Default::default(),
            snapshots: Arc::new(SnapshotAdmission::default()),
        }
    }
    /// Return the exact bound resource including generation.
    pub const fn resource(&self) -> ResourceKey {
        self.resource
    }
    /// Return accepted-turn revision; rejected batches never advance it.
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    /// Return cumulative typed loss, without product-copy or foreign-text formatting.
    pub const fn diagnostics(&self) -> TranscriptDiagnostics {
        self.diagnostics
    }
    /// Return retained history counts: records, UTF-8 bytes and style runs.
    pub fn retained_counts(&self) -> (usize, usize, usize) {
        (self.records.len(), self.text_bytes, self.style_runs)
    }
    /// Return current observable history/scratch/container allocation capacities.
    /// Snapshot-pinned previous versions are separately bounded by snapshot policy.
    pub fn allocated_bytes(&self) -> usize {
        self.records.capacity() * size_of::<Arc<TranscriptRecord>>()
            + self
                .records
                .iter()
                .map(|r| r.allocated_bytes() + size_of::<TranscriptRecord>())
                .sum::<usize>()
            + self
                .streams
                .iter()
                .map(|s| {
                    s.line.text.capacity()
                        + s.line.runs.capacity() * size_of::<StyleRun>()
                        + size_of::<Stream>()
                })
                .sum::<usize>()
    }
    /// Validate identity, sequence, gap, EOF and overflow before any mutation.
    /// At most32KiB input is inspected. Unsupported terminal commands are discarded;
    /// text/style bounds truncate the current record until a delimiter or CR update.
    /// Checked revision/sequence/ID exhaustion rejects the whole turn unchanged.
    pub fn apply(&mut self, batch: &OutputBatch) -> Result<TranscriptChange, TranscriptError> {
        let index = batch.stream().index();
        if batch.resource() != self.resource {
            return Err(TranscriptError::ForeignResource);
        }
        let stream = &self.streams[index];
        if stream.ended {
            return Err(TranscriptError::StreamEnded);
        }
        let expected = stream
            .sequence
            .checked_add(1)
            .and_then(|v| v.checked_add(batch.gap().map_or(0, OutputGap::skipped_batches)))
            .ok_or(TranscriptError::Exhausted)?;
        if batch.sequence().get() != expected {
            return Err(TranscriptError::Sequence);
        }
        let revision = self.revision.checked_add(1).ok_or(TranscriptError::Exhausted)?;
        // At most one new ID per input byte, plus sealing pending UTF-8/empty EOF.
        self.next_id
            .checked_add(batch.bytes().len() as u64 + 2)
            .ok_or(TranscriptError::Exhausted)?;
        let old_next = self.next_id;
        let previous = self.streams.each_ref().map(|s| s.line.id);
        let mut change = TranscriptChange {
            revision,
            appended: None,
            replaced: [None; 2],
            trimmed_through: None,
            diagnostics: self.diagnostics,
        };
        self.streams[index].sequence = expected;
        if let Some(gap) = batch.gap() {
            let (tail, malformed) = self.streams[index].parser.finish();
            self.add(Counter::Malformed, malformed);
            if let Some(character) = tail {
                self.add(Counter::Unicode, 1);
                self.character(index, character, &mut change)?;
            }
            self.finish_line(index, false, &mut change)?;
            self.streams[index].parser.reset();
            self.streams[index].line.cr = false;
            self.add(Counter::Dropped(index), gap.dropped_bytes());
            self.add(Counter::Skipped(index), gap.skipped_batches());
        }
        for byte in batch.bytes() {
            let (decoded, malformed) = self.streams[index].parser.push(*byte);
            self.add(Counter::Malformed, malformed);
            self.add(Counter::Unicode, decoded.replacements);
            for character in &decoded.chars[..decoded.length] {
                self.character(index, *character, &mut change)?;
            }
        }
        if batch.end() {
            let (tail, malformed) = self.streams[index].parser.finish();
            self.add(Counter::Malformed, malformed);
            if let Some(character) = tail {
                self.add(Counter::Unicode, 1);
                self.character(index, character, &mut change)?;
            }
            self.finish_line(index, false, &mut change)?;
            self.streams[index].ended = true;
        } else {
            self.publish(index, false, &mut change)?;
        }
        for (index, id) in previous.into_iter().enumerate() {
            if let Some(id) = id
                && self.records.iter().any(|r| r.id() == id)
            {
                // Only the affected stream can replace its prior current record.
                if index == batch.stream().index() {
                    change.replaced[index] = Some(id);
                }
            }
        }
        if self.next_id != old_next {
            change.appended = Some((RecordId::new(old_next)?, RecordId::new(self.next_id - 1)?));
        }
        self.revision = revision;
        change.diagnostics = self.diagnostics;
        Ok(change)
    }
    fn add(&mut self, counter: Counter, value: u64) {
        let target = match counter {
            Counter::Dropped(i) => &mut self.diagnostics.dropped_bytes[i],
            Counter::Skipped(i) => &mut self.diagnostics.skipped_batches[i],
            Counter::Malformed => &mut self.diagnostics.malformed_escapes,
            Counter::Unicode => &mut self.diagnostics.unicode_replacements,
            Counter::TruncatedBytes => &mut self.diagnostics.truncated_bytes,
            Counter::TruncatedRecords => &mut self.diagnostics.truncated_records,
            Counter::TrimmedRecords => &mut self.diagnostics.trimmed_records,
            Counter::TrimmedBytes => &mut self.diagnostics.trimmed_bytes,
        };
        if let Some(sum) = target.checked_add(value) {
            *target = sum;
        } else {
            *target = u64::MAX;
            self.diagnostics.counter_saturated = true;
        }
    }
}
#[cfg(test)]
mod tests;
enum Counter {
    Dropped(usize),
    Skipped(usize),
    Malformed,
    Unicode,
    TruncatedBytes,
    TruncatedRecords,
    TrimmedRecords,
    TrimmedBytes,
}
