// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Keep native metadata outside the actual shared state and its last Arc control.

use super::{Shared, metadata::Metadata};
#[cfg(windows)]
use std::sync::atomic::AtomicUsize;
use std::{ops::Deref, sync::Arc};

/// Every private shared alias carries the same outside metadata guard.
/// Field order frees the state/control allocation before the guard can return capacity.
#[derive(Clone)]
pub(crate) struct SharedRef {
    value: Arc<Shared>,
    pub(crate) metadata: Metadata,
}

// Pinned Rust 1.93.1 ArcInner is repr(C), including payload alignment/padding.
#[repr(C)]
#[cfg(windows)]
struct ControlLayout {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: Shared,
}

impl SharedRef {
    /// Construct the sole state/control pair inside its actual admitted reservation.
    pub(crate) fn new(value: Shared, metadata: Metadata) -> Self {
        Self { value: Arc::new(value), metadata }
    }

    /// Match actual state allocation; numerical identities are insufficient.
    pub(crate) fn same_allocation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.value, &other.value)
    }

    /// Report actual compiled state/control bytes for the fixed metadata preflight.
    #[cfg(windows)]
    pub(crate) const fn control_storage_bytes() -> usize {
        size_of::<ControlLayout>()
    }
}

impl Deref for SharedRef {
    type Target = Shared;

    /// Borrow state without exposing either private Arc or a Weak constructor.
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
