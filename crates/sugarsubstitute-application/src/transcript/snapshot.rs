// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit bounded visible snapshots without exposing clonable history owners.
use super::TranscriptStore;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::{lifecycle::ResourceKey, transcript::*};
/// Maximum records referenced by a visible snapshot.
pub const MAX_SNAPSHOT_RECORDS: usize = 128;
/// Maximum UTF-8 text pinned by a visible snapshot.
pub const MAX_SNAPSHOT_BYTES: usize = 256 * 1024;
const MAX_SNAPSHOTS: usize = 4;
#[derive(Default)]
pub(super) struct SnapshotAdmission {
    live: AtomicUsize,
}

/// Immutable borrowed-access history view with one of four retained snapshot credits.
/// No Clone or Arc extraction is exposed; references cannot outlive this owner.
/// ```compile_fail
/// use sugarsubstitute_application::transcript::TranscriptSnapshot;
/// fn duplicate(snapshot: TranscriptSnapshot) -> TranscriptSnapshot {
///     snapshot.clone()
/// }
/// ```
pub struct TranscriptSnapshot {
    resource: ResourceKey,
    revision: u64,
    records: Vec<Arc<TranscriptRecord>>,
    admission: Arc<SnapshotAdmission>,
    diagnostics: TranscriptDiagnostics,
}
impl TranscriptSnapshot {
    /// Return the resource generation of this immutable view.
    pub const fn resource(&self) -> ResourceKey {
        self.resource
    }
    /// Return the revision sampled when the view was admitted.
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    /// Borrow immutable records; private shared owners cannot be extracted.
    pub fn records(&self) -> impl ExactSizeIterator<Item = &TranscriptRecord> {
        self.records.iter().map(Arc::as_ref)
    }
    /// Return loss facts sampled with the same revision.
    pub const fn diagnostics(&self) -> TranscriptDiagnostics {
        self.diagnostics
    }
    /// Return observable pinned record/container capacities in bytes.
    /// Shared records are counted in each view; summing is a conservative bound.
    pub fn allocated_bytes(&self) -> usize {
        size_of::<Self>()
            + self.records.capacity() * size_of::<Arc<TranscriptRecord>>()
            + self
                .records
                .iter()
                .map(|r| {
                    r.allocated_bytes() + size_of::<TranscriptRecord>() + 2 * size_of::<usize>()
                })
                .sum::<usize>()
    }
}
impl Drop for TranscriptSnapshot {
    fn drop(&mut self) {
        // Return admission only after old-version pins have actually been released.
        self.records.clear();
        self.admission.live.fetch_sub(1, Ordering::AcqRel);
    }
}
impl TranscriptStore {
    /// Pin a bounded visible range beginning at/after `first`, or the oldest record.
    /// Require1..=128 records and one of four credits. Stop before exceeding256KiB
    /// text. A missing/trimmed anchor selects its next retained successor.
    /// Rejection leaves all counters and history unchanged; snapshots survive store drop.
    pub fn snapshot(
        &self,
        first: Option<RecordId>,
        max_records: usize,
    ) -> Result<TranscriptSnapshot, TranscriptError> {
        if max_records == 0 || max_records > MAX_SNAPSHOT_RECORDS {
            return Err(TranscriptError::Bound);
        }
        self.snapshots
            .live
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |live| {
                (live < MAX_SNAPSHOTS).then_some(live + 1)
            })
            .map_err(|_| TranscriptError::SnapshotCapacity)?;
        let mut records = Vec::with_capacity(max_records);
        let mut bytes = 0;
        for record in self.records.iter().filter(|r| first.is_none_or(|id| r.id() >= id)) {
            if records.len() == max_records || bytes + record.text().len() > MAX_SNAPSHOT_BYTES {
                break;
            }
            bytes += record.text().len();
            records.push(record.clone());
        }
        Ok(TranscriptSnapshot {
            resource: self.resource(),
            revision: self.revision(),
            records,
            admission: self.snapshots.clone(),
            diagnostics: self.diagnostics(),
        })
    }
}
