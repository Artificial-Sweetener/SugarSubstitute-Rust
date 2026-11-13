// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind intentional provider observations to one actual fake native callback on its current thread.
use std::{cell::Cell, sync::Arc};
thread_local! {
    static CURRENT: Cell<Option<usize>> = const { Cell::new(None) };
}
/// Observe the external callback before its genuine context provider is invoked.
pub(in crate::diagnostic_storage) trait NativeControlProbe:
    Send + Sync
{
    /// Check callback ownership without executing the provider or changing storage state.
    fn before_context(&self);
}
/// Restore the preceding callback scope on normal return and provider unwind.
pub(in crate::diagnostic_storage) struct NativeContext {
    probe: Option<Arc<dyn NativeControlProbe>>,
    previous: Option<usize>,
}
impl NativeContext {
    /// Mark only this thread's synchronous callback, preserving nested callback ownership.
    pub(in crate::diagnostic_storage) fn new(probe: Option<Arc<dyn NativeControlProbe>>) -> Self {
        let previous = probe.as_ref().and_then(|probe| {
            CURRENT.with(|current| current.replace(Some(Arc::as_ptr(probe) as *const () as usize)))
        });
        let scope = Self { probe, previous };
        if let Some(probe) = &scope.probe {
            probe.before_context();
        }
        scope
    }
    /// Report whether this callback has a configured boundary probe.
    pub(in crate::diagnostic_storage) fn observed(&self) -> bool {
        self.probe.is_some()
    }
}
impl Drop for NativeContext {
    fn drop(&mut self) {
        if self.probe.is_some() {
            CURRENT.with(|current| current.set(self.previous));
        }
    }
}
/// Compare actual current callback identity; other execution workers cannot borrow its scope.
pub(in crate::diagnostic_storage) fn is_current(identity: usize) -> bool {
    CURRENT.with(|current| current.get() == Some(identity))
}
