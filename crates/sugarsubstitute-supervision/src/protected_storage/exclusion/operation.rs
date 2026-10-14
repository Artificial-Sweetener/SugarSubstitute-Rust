// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Take and restore original record state without holding locks across providers.
use super::*;
use std::{
    ops::{Deref, DerefMut},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

/// Exclude another operation until every partial output is restored.
pub(crate) struct Operation<'a>(&'a AtomicBool);
impl<'a> Operation<'a> {
    /// Acquire a nonblocking fence; an in-flight owner remains unchanged on rejection.
    pub(crate) fn begin<const N: usize>(file: &'a HeldFile<N>) -> Result<Self, ClaimError> {
        Self::acquire(&file.operating, &file.handle.budget())
    }
    /// Fence disjoint mutable handle fields without borrowing their native storage.
    pub(super) fn acquire(flag: &'a AtomicBool, budget: &Budget) -> Result<Self, ClaimError> {
        flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| budget.rejection("record_operation_pending"))?;
        Ok(Self(flag))
    }
}
impl Drop for Operation<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Own the identical state outside its storage lock and reinstall it on unwind.
/// The enclosing operation fence prevents concurrent state replacement.
pub(crate) struct Loan<'a, T: Default> {
    value: T,
    storage: &'a Mutex<T>,
}
impl<'a, T: Default> Loan<'a, T> {
    /// Take only under the caller's retained operation fence.
    pub(crate) fn take(storage: &'a Mutex<T>, _operation: &Operation<'_>) -> Self {
        let value = std::mem::take(&mut *lock(storage));
        Self { value, storage }
    }
}
impl<T: Default> Deref for Loan<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}
impl<T: Default> DerefMut for Loan<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}
impl<T: Default> Drop for Loan<'_, T> {
    fn drop(&mut self) {
        *lock(self.storage) = std::mem::take(&mut self.value);
    }
}
