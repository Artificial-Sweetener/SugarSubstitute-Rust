// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Couple child control aliases to the one original source/control storage claim.

use super::control::DesktopCaptureCustody;
use std::{
    ops::Deref,
    sync::{Arc, atomic::AtomicUsize},
};
use sugarsubstitute_contracts::execution::MemoryClaim;

pub(super) const SOURCE_CONTROL_BYTES: u64 = 16_384;

/// Private aliases share the identical claim; cloning issues no new memory claim.
/// No Weak/control projection escapes, and every drop consumes into_inner so the
/// last private charge control is freed before the returned claim can release.
#[derive(Clone)]
pub(crate) struct ChildMetadata(Option<Arc<MemoryClaim>>);

impl ChildMetadata {
    /// Install only the actual original admission's preclaimed metadata guard.
    /// Caller preflights all concrete child storage before this control allocation.
    pub(super) fn new(claim: MemoryClaim) -> Self {
        Self(Some(Arc::new(claim)))
    }

    /// Count the sole actual private charge control for the compiled preflight.
    pub(super) const fn storage_bytes() -> usize {
        size_of::<ChargeControlLayout>()
    }
}

impl Drop for ChildMetadata {
    /// Release only after the final actual private charge control has been freed.
    fn drop(&mut self) {
        if let Some(owner) = self.0.take() {
            drop(Arc::into_inner(owner));
        }
    }
}

#[repr(C)]
struct ChargeControlLayout {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: MemoryClaim,
}

/// An actual child custody/control allocation with inseparable outside metadata.
/// All private aliases are this closed value; there is no bare Arc/Weak export.
/// Field order destroys value/control before dropping its outside charge alias.
#[derive(Clone)]
pub(crate) struct DesktopCaptureCustodyRef {
    value: Arc<DesktopCaptureCustody>,
    _metadata: ChildMetadata,
}

impl DesktopCaptureCustodyRef {
    /// Allocate the actual control only after its concrete compiled layout fits.
    pub(super) fn new(value: DesktopCaptureCustody, metadata: ChildMetadata) -> Self {
        Self { value: Arc::new(value), _metadata: metadata }
    }

    /// Compare the same retained allocation without exporting a cloneable Arc.
    pub(crate) fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.value, &other.value)
    }

    /// Borrow the same outside guard for finite cancellation/shutdown sampling.
    /// This shares the one original claim and grants no admission/effect permission.
    pub(crate) fn metadata(&self) -> ChildMetadata {
        self._metadata.clone()
    }

    /// Count this actual control allocation once in the fixed metadata partition.
    pub(super) const fn storage_bytes() -> usize {
        size_of::<CustodyControlLayout>()
    }
}

impl Deref for DesktopCaptureCustodyRef {
    type Target = DesktopCaptureCustody;

    /// Borrow current state from the guarded actual allocation, preserving its charge.
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl AsRef<DesktopCaptureCustody> for DesktopCaptureCustodyRef {
    /// Borrow the real retained control without exposing its Arc or a new owner.
    fn as_ref(&self) -> &DesktopCaptureCustody {
        &self.value
    }
}

#[repr(C)]
struct CustodyControlLayout {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: DesktopCaptureCustody,
}

/// Keep the child metadata outside an actual cleanup Arc borrowed by Core dispatch.
/// The cleanup value/control drops before its final metadata alias, including panic.
pub(crate) struct NativeCustodyLoan {
    pub(crate) custody: Arc<crate::capture_attach::control::Custody>,
    _metadata: Option<ChildMetadata>,
}

impl NativeCustodyLoan {
    /// Snapshot only actual retained cleanup and optional actual child metadata.
    /// Parent custody preserves its existing owner; child claims are never duplicated.
    pub(crate) fn new(
        custody: Arc<crate::capture_attach::control::Custody>,
        desktop: Option<&DesktopCaptureCustodyRef>,
    ) -> Self {
        Self { custody, _metadata: desktop.map(DesktopCaptureCustodyRef::metadata) }
    }
}

impl Deref for NativeCustodyLoan {
    type Target = crate::capture_attach::control::Custody;

    /// Borrow for the existing cleanup request callback outside Core's lock.
    fn deref(&self) -> &Self::Target {
        &self.custody
    }
}
