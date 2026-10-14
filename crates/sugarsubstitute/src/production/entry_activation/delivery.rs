// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fund and retain one complete backend return before issuing another producer call.
use super::{
    CaptureRegistration, CoordinatorBackend, Result, RetainedRecord, forwarding::Forwarded,
    progress::with_parent,
};
use crate::production::entry_runner::EntryResources;
use sugarsubstitute_contracts::{execution::MemoryClaim, lifecycle::ResourceTransition};

/// The same original projection pays for retained recipient accounting before any
/// producer effect. The complete return precedes the outside claim in destruction.
pub(super) struct TransitionDelivery {
    pending: Box<Option<Forwarded>>,
    _storage: MemoryClaim,
}
impl TransitionDelivery {
    pub(super) const fn storage_bytes() -> u64 {
        size_of::<Option<Forwarded>>() as u64
    }
    pub(super) fn new(storage: MemoryClaim) -> Result<Self> {
        if storage.bytes() < Self::storage_bytes() {
            return Err("Backend delivery storage was not admitted".into());
        }
        Ok(Self { pending: Box::new(None), _storage: storage })
    }
    pub(super) fn pending(&self) -> bool {
        self.pending.is_some()
    }
    /// Check the finite slot before invoking any producer; own its result before
    /// another recipient/provider can fail. Native effects are never replayed here.
    pub(super) fn produce(
        &mut self,
        producer: impl FnOnce() -> Result<ResourceTransition>,
    ) -> Result<()> {
        if self.pending() {
            return Err("Previous actual backend return awaits delivery".into());
        }
        *self.pending = Some(Forwarded::new(producer()?));
        Ok(())
    }
    /// A failed producer may retain its actual return before the failure. Never
    /// take that owner while the one charged delivery slot remains occupied.
    pub(super) fn capture_pending(&mut self, source: impl FnOnce() -> Option<ResourceTransition>) {
        if !self.pending() {
            *self.pending = source().map(Forwarded::new);
        }
    }
    pub(super) fn route(
        &mut self,
        records: &mut [Option<RetainedRecord>; 64],
        backend: &mut CoordinatorBackend,
        capture: Option<&CaptureRegistration>,
        resources: &mut EntryResources<'_>,
    ) -> Result<()> {
        self.deliver(|forwarded| {
            for record in records.iter_mut().flatten().map(|held| held.record.as_mut()) {
                if let Some(retired) = &record.retired {
                    forwarded.retire(retired)?;
                } else if record.parent.is_some() && !record.closed {
                    forwarded.deliver(record.run, |transition| {
                        with_parent(record, backend, capture, resources, |parent, borrowed| {
                            Ok(parent.backend_transition(transition, borrowed)?)
                        })
                    })?;
                }
            }
            Ok(())
        })
    }
    fn deliver(&mut self, recipients: impl FnOnce(&mut Forwarded) -> Result<()>) -> Result<()> {
        let Some(forwarded) = self.pending.as_mut() else { return Ok(()) };
        let result = recipients(forwarded);
        if forwarded.complete() {
            *self.pending = None;
        }
        result
    }
}

#[cfg(test)]
#[path = "delivery_tests.rs"]
mod tests;
