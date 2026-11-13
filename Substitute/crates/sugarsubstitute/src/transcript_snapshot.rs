// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain original output admission outside shared view and claim control allocations.
use std::sync::Arc;
use sugarsubstitute_application::transcript::TranscriptSnapshot;
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
/// Retain the original allocation and reservation across shared output views.
pub struct SourceCustody {
    claim: Option<Arc<MemoryClaim>>,
    _reservation: MemoryLease,
}
impl SourceCustody {
    /// Retain admitted source storage until the last view is released.
    pub fn new(claim: MemoryClaim, reservation: MemoryLease) -> Self {
        Self { claim: Some(Arc::new(claim)), _reservation: reservation }
    }
    fn bytes(&self) -> u64 {
        self.claim.as_ref().map_or(0, |claim| claim.bytes())
    }
}
impl Clone for SourceCustody {
    fn clone(&self) -> Self {
        Self { claim: self.claim.clone(), _reservation: self._reservation.clone() }
    }
}
impl Drop for SourceCustody {
    fn drop(&mut self) {
        // into_inner frees the last control allocation before releasing the original credit.
        if let Some(claim) = self.claim.take() {
            drop(Arc::into_inner(claim));
        }
    }
}
/// Closed shared render source constructed only on the admitted parser worker.
/// Cloning retains the same pin and outside custody; it performs no allocation.
/// Final Drop frees the control header and transcript before returning original credit.
pub struct OutputSnapshot {
    view: Arc<TranscriptSnapshot>,
    source: SourceCustody,
}
impl OutputSnapshot {
    /// Own an immutable transcript with its original source funding.
    pub fn new(view: TranscriptSnapshot, source: SourceCustody) -> Self {
        Self { view: Arc::new(view), source }
    }
    /// Borrow genuine immutable text without extracting its pin or allocation custody.
    pub fn transcript(&self) -> &TranscriptSnapshot {
        &self.view
    }
    /// Observe original source bytes counted once across every clone.
    pub fn source_bytes(&self) -> u64 {
        self.source.bytes()
    }
}
impl Clone for OutputSnapshot {
    fn clone(&self) -> Self {
        Self { view: self.view.clone(), source: self.source.clone() }
    }
}

/// Borrow the actual immutable pin while original source funding remains retained here.
impl sugarsubstitute_desktop::ConsoleText for OutputSnapshot {
    fn resource(&self) -> sugarsubstitute_contracts::lifecycle::ResourceKey {
        self.transcript().resource()
    }
    fn revision(&self) -> u64 {
        self.transcript().revision()
    }
    fn len(&self) -> usize {
        self.transcript().records().len()
    }
    fn record(
        &self,
        index: usize,
    ) -> Option<&sugarsubstitute_contracts::transcript::TranscriptRecord> {
        self.transcript().records().nth(index)
    }
}
