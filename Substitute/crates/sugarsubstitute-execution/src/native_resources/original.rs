// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the unique admitted symbol context allocation inside its original metadata guard.
use std::{
    ops::Deref,
    sync::{Arc, atomic::AtomicUsize},
};
use sugarsubstitute_contracts::execution::{JobContext, NativeHandleClaim};

/// Each closed clone frees its context Arc/control before the outside native
/// partition guard can release the original fixed4096 metadata claim.
/// No raw Arc/Weak projection or extra allocation is available to consumers.
#[derive(Clone)]
pub(crate) struct SymbolOriginalContext {
    value: Arc<JobContext>,
    _guard: Arc<NativeHandleClaim>,
}
#[repr(C)]
struct Control {
    strong: AtomicUsize,
    weak: AtomicUsize,
    value: JobContext,
}
impl SymbolOriginalContext {
    /// Construct only after actual Core validates this context and partition.
    #[cfg(any(windows, test))]
    pub(crate) fn new(context: &JobContext, guard: Arc<NativeHandleClaim>) -> Self {
        Self { value: Arc::new(context.clone()), _guard: guard }
    }
    /// Count the one original allocation; every later clone is allocation-free.
    pub(crate) const fn control_storage_bytes() -> usize {
        size_of::<Control>()
    }
}
impl AsRef<JobContext> for SymbolOriginalContext {
    /// Borrow actual original ports without projecting the private Arc or guard.
    fn as_ref(&self) -> &JobContext {
        &self.value
    }
}
impl Deref for SymbolOriginalContext {
    type Target = JobContext;
    /// Retain the outside metadata guard for the full borrow of original context.
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}
