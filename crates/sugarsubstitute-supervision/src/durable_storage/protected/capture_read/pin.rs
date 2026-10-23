// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain original source credit through the final actual capture reader pin.
use super::super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
struct Counts {
    readers: AtomicUsize,
    open: AtomicBool,
}
#[repr(C)]
struct Control {
    strong: AtomicUsize,
    weak: AtomicUsize,
    value: Counts,
}
pub(crate) struct ReadPins {
    value: Option<Arc<Counts>>,
    budget: Budget,
}
pub(crate) struct ReadPin {
    pins: ReadPins,
}
impl ReadPins {
    /// Borrow the same closed source control and outside original claim without allocation.
    pub(in crate::durable_storage::protected) fn borrow(&self) -> Self {
        Self { value: self.value.clone(), budget: self.budget.clone() }
    }
    /// Match only this actual retained source allocation, never caller identity data.
    pub(in crate::durable_storage::protected) fn matches_budget(&self, budget: &Budget) -> bool {
        match (&self.budget, budget) {
            (Budget::Protected(left), Budget::Protected(right)) => Arc::ptr_eq(left, right),
            (Budget::Slot(left), Budget::Slot(right)) => Arc::ptr_eq(left, right),
            (Budget::Core(left), Budget::Core(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }

    pub(crate) fn open(&self) -> bool {
        self.value.as_ref().is_some_and(|value| value.open.load(Ordering::Acquire))
    }
    pub(crate) fn same(&self, other: &Self) -> bool {
        self.value
            .as_ref()
            .zip(other.value.as_ref())
            .is_some_and(|(left, right)| Arc::ptr_eq(left, right))
    }

    pub(in crate::durable_storage::protected) const fn control_bytes() -> usize {
        size_of::<Control>()
    }
    pub(in crate::durable_storage::protected) fn new(budget: Budget) -> Self {
        Self {
            value: Some(Arc::new(Counts {
                readers: AtomicUsize::new(0),
                open: AtomicBool::new(true),
            })),
            budget,
        }
    }
    pub(in crate::durable_storage::protected) fn count(&self) -> usize {
        self.value.as_ref().map_or(0, |value| value.readers.load(Ordering::Acquire))
    }
    pub(in crate::durable_storage::protected) fn close(&self) {
        if let Some(value) = &self.value {
            value.open.store(false, Ordering::Release);
        }
    }
    pub(in crate::durable_storage::protected) fn acquire(&self) -> Result<ReadPin, ClaimError> {
        let value = self
            .value
            .as_ref()
            .ok_or_else(|| self.budget.rejection("capture_read_source_closed"))?;
        if !value.open.load(Ordering::Acquire) {
            return Err(self.budget.rejection("capture_read_source_closed"));
        }
        value
            .readers
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                old.checked_add(1).filter(|next| *next <= 4)
            })
            .map_err(|_| self.budget.rejection("capture_read_source_bound"))?;
        let pin = ReadPin {
            pins: ReadPins { value: Some(Arc::clone(value)), budget: self.budget.clone() },
        };
        if !value.open.load(Ordering::Acquire) {
            return Err(self.budget.rejection("capture_read_source_closed"));
        }
        Ok(pin)
    }
}
impl ReadPin {
    pub(crate) fn fence(&self) -> ReadPins {
        ReadPins { value: self.pins.value.clone(), budget: self.pins.budget.clone() }
    }

    pub(in crate::durable_storage::protected) fn open(&self) -> bool {
        self.pins.value.as_ref().is_some_and(|value| value.open.load(Ordering::Acquire))
    }
}
impl Drop for ReadPins {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            let _consumed = Arc::into_inner(value);
        }
    }
}
impl Drop for ReadPin {
    fn drop(&mut self) {
        if let Some(value) = &self.pins.value {
            value.readers.fetch_sub(1, Ordering::AcqRel);
        }
    }
}
