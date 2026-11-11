// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep an abandoned armed export's complete allocation and credit pinned.
use std::{
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
};
/// Keep complete abandoned armed data and its actual admission charge pinned.
pub(crate) struct Retained<T> {
    value: ManuallyDrop<Box<T>>,
    closed: bool,
}
impl<T> Retained<T> {
    /// Allocate the sole fixed owner value from already admitted original credit.
    pub(crate) fn new(value: T) -> Self {
        Self { value: ManuallyDrop::new(Box::new(value)), closed: false }
    }
    /// Mark authoritative checked closure before normal allocation destruction.
    pub(crate) fn release(&mut self) {
        self.closed = true;
    }
}
impl<T> Deref for Retained<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}
impl<T> DerefMut for Retained<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}
impl<T> Drop for Retained<T> {
    fn drop(&mut self) {
        if self.closed {
            // SAFETY: Sole boxed value is destroyed once, only after its authoritative
            // owner observes checked closure. Armed values keep their complete allocation.
            unsafe {
                ManuallyDrop::drop(&mut self.value);
            }
        }
    }
}
