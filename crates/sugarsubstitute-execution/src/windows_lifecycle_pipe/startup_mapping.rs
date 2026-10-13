// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Borrow immutable native startup bytes while retaining their original storage claim.

use crate::{coordinator_entry::EntryTransportError, native::Signal};
use std::{
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::MemoryClaim;

/// The sole native section worker owns physical map retirement and the two output loans.
/// An uncertain native owner retains this entire value with its original view/handles.
pub(super) struct StartupMapping {
    storage: StorageRef,
    issued: u8,
}

/// Every private storage alias retains its charge outside the actual storage Arc.
/// Field order frees the value/control before the charge's final private control.
#[derive(Clone)]
struct StorageRef {
    value: Arc<Storage>,
    _charge: Charge,
}

/// No native close, claim or strong native/Core owner is held inside this payload.
struct Storage {
    address: AtomicUsize,
    length: usize,
    loans: AtomicUsize,
    signal: Arc<Signal>,
}

/// No Weak/raw Arc escape exists. Every alias consumes into_inner on drop.
#[derive(Clone)]
struct Charge(Option<Arc<MemoryClaim>>);

#[repr(C)]
struct StorageControlLayout {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: Storage,
}

#[repr(C)]
struct ChargeControlLayout {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: MemoryClaim,
}

impl Drop for Charge {
    /// Free its last private control allocation before the returned claim can release.
    fn drop(&mut self) {
        if let Some(owner) = self.0.take() {
            drop(Arc::into_inner(owner));
        }
    }
}

impl StartupMapping {
    /// Reserve actual storage/control under the original captured metadata partition.
    /// The claim is moved once, before native mapping; native output is adopted later.
    pub(super) fn new(claim: MemoryClaim, signal: Arc<Signal>) -> Self {
        let charge = Charge(Some(Arc::new(claim)));
        Self {
            storage: StorageRef {
                value: Arc::new(Storage {
                    address: AtomicUsize::new(0),
                    length: 0,
                    loans: AtomicUsize::new(0),
                    signal,
                }),
                _charge: charge,
            },
            issued: 0,
        }
    }

    /// Adopt the actual read-only view into its already retained output cell.
    ///
    /// # Safety
    /// `address..address+length` is the actual immutable view owned by this exact Section.
    /// Its same-worker native owner retains this whole value through uncertainty and
    /// unmaps only when `can_unmap()` returns true. No output loan exists before adoption.
    pub(super) unsafe fn adopt(
        &mut self,
        address: usize,
        length: usize,
    ) -> Result<(), EntryTransportError> {
        if self.issued != 0 || address == 0 || length == 0 || address.checked_add(length).is_none()
        {
            return Err(EntryTransportError::Malformed);
        }
        let state = Arc::get_mut(&mut self.storage.value).ok_or(EntryTransportError::Malformed)?;
        if state.address.load(Ordering::Acquire) != 0 {
            return Err(EntryTransportError::Malformed);
        }
        state.length = length;
        state.address.store(address, Ordering::Release);
        Ok(())
    }

    /// Issue one of the two preclaimed original loans on the sole physical-owner worker.
    pub(super) fn loan(
        &mut self,
        range: Range<usize>,
    ) -> Result<StartupViewLoan, EntryTransportError> {
        let state = &self.storage.value;
        if self.issued == 2
            || range.start >= range.end
            || range.end > state.length
            || state.address.load(Ordering::Acquire) == 0
        {
            return Err(EntryTransportError::Malformed);
        }
        self.issued += 1;
        state.loans.fetch_add(1, Ordering::AcqRel);
        Ok(StartupViewLoan { alias: LoanAlias { storage: self.storage.clone() }, range })
    }

    /// Observe physical borrow permission only; zero loans never returns metadata capacity.
    pub(super) fn can_unmap(&self) -> bool {
        self.storage.value.loans.load(Ordering::Acquire) == 0
    }

    /// Invalidate the cache only after actual checked unmap with both loans returned.
    pub(super) fn unmapped(&mut self) {
        self.storage.value.address.store(0, Ordering::Release);
    }

    /// Report both actual private Arc controls and value layouts for preflight.
    pub(super) const fn control_storage_bytes() -> usize {
        size_of::<StorageControlLayout>()
            + size_of::<ChargeControlLayout>()
            + size_of::<Self>()
            + 2 * size_of::<StartupViewLoan>()
            + crate::desktop_capture::InheritedCaptureAttachInput::storage_bytes()
            + crate::windows_lifecycle_pipe::captured::child::storage_bytes()
            + size_of::<crate::native::Signal>()
            + 2 * size_of::<AtomicUsize>()
    }
}

/// One nonclone bounded original view loan; Drop returns no native retirement authority.
pub(crate) struct StartupViewLoan {
    alias: LoanAlias,
    range: Range<usize>,
}

impl StartupViewLoan {
    /// Borrow immutable bytes for the sole Launch decoder, without granting native authority.
    pub(crate) fn bytes(&self) -> Result<&[u8], EntryTransportError> {
        let state = &self.alias.storage.value;
        let address = state.address.load(Ordering::Acquire);
        let start = address.checked_add(self.range.start).ok_or(EntryTransportError::Malformed)?;
        if address == 0 || self.range.end > state.length {
            return Err(EntryTransportError::Malformed);
        }
        // SAFETY: this actual nonclone loan keeps the physical borrow count nonzero.
        // The original read-only view cannot be unmapped until this value is dropped.
        Ok(unsafe { std::slice::from_raw_parts(start as *const u8, self.range.len()) })
    }
}

/// Retain only the genuine original physical borrow and its outside storage charge.
/// No bytes/range, additional allocation, new count, or native retirement authority exists.
pub(crate) struct StartupViewRetention {
    _alias: LoanAlias,
}
/// Exactly one original physical borrow; outside charge outlives its real Arc drop.
struct LoanAlias {
    storage: StorageRef,
}
impl StartupViewLoan {
    /// Move the same actual loan to cache-only capture facts after authentication.
    /// Only the startup view still needs a byte range; this returns no borrow/count.
    pub(crate) fn into_retention(self) -> StartupViewRetention {
        StartupViewRetention { _alias: self.alias }
    }
}
impl Drop for LoanAlias {
    /// End physical byte borrowing before waking; the outside claim still covers live controls.
    fn drop(&mut self) {
        self.storage.value.loans.fetch_sub(1, Ordering::AcqRel);
        self.storage.value.signal.complete();
    }
}

#[cfg(test)]
#[path = "startup_mapping_tests.rs"]
mod tests;
