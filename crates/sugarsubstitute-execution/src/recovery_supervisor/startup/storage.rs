// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Free receiver/source allocations and their private controls before returning original capacity.
use super::Source;
use std::{ops::Deref, sync::Arc};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
/// Every source alias retains an outside claim; neither a Weak nor raw Arc escapes.
#[derive(Clone)]
pub(crate) struct SourceRef {
    value: Arc<Source>,
    _charge: Charge,
}
impl SourceRef {
    pub(crate) fn same_allocation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.value, &other.value)
    }
    pub(super) fn publish(value: Source, charge: Charge) -> Self {
        Self { value: Arc::new(value), _charge: charge }
    }
}
impl Deref for SourceRef {
    type Target = Source;
    fn deref(&self) -> &Source {
        &self.value
    }
}
/// Private outside guard; its final control is destroyed before its payload releases credit.
#[derive(Clone)]
pub(super) struct Charge(Option<Arc<Charged>>);
struct Charged {
    _claim: MemoryClaim,
    _memory: MemoryLease,
}
impl Charge {
    pub(super) fn new(claim: MemoryClaim, memory: MemoryLease) -> Self {
        Self(Some(Arc::new(Charged { _claim: claim, _memory: memory })))
    }
    pub(super) const fn storage_bytes() -> usize {
        size_of::<Charged>() + 2 * size_of::<usize>()
    }
}
impl Drop for Charge {
    fn drop(&mut self) {
        if let Some(owner) = self.0.take() {
            drop(Arc::into_inner(owner));
        }
    }
}
