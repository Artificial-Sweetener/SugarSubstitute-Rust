// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Count reentrant native host callbacks so retirement never releases a borrowed controller.

use super::*;
use std::cell::Cell;

/// Retain a removal wake only while a reentrant native callback actually owns the controller.
#[derive(Default)]
pub(super) struct CallbackFence {
    active: Cell<usize>,
    waiting: Cell<bool>,
}

impl CallbackFence {
    /// Count the exact in-flight host operation before calling into WebView2.
    pub(super) fn enter<'a>(
        &'a self,
        events: &'a Events,
    ) -> Result<CallbackScope<'a>, DesktopError> {
        let active = self.active.get().checked_add(1).ok_or_else(|| {
            DesktopError::message("composed_browser_callback", "native callback depth exhausted")
        })?;
        self.active.set(active);
        Ok(CallbackScope { fence: self, events })
    }
}

/// Return one callback's custody and wake a deferred removal after the last return.
pub(super) struct CallbackScope<'a> {
    fence: &'a CallbackFence,
    events: &'a Events,
}

impl Drop for CallbackScope<'_> {
    fn drop(&mut self) {
        let active = self.fence.active.get() - 1;
        self.fence.active.set(active);
        if active == 0 && self.fence.waiting.replace(false) {
            self.events.wake();
        }
    }
}

/// Run a native removal only after every callback using the controller has returned.
/// A deferred removal returns false and is woken by the last returning callback.
pub(super) fn retire_when_quiet(
    callbacks: &CallbackFence,
    remove: impl FnOnce() -> Result<(), DesktopError>,
) -> Result<bool, DesktopError> {
    if callbacks.active.get() != 0 {
        callbacks.waiting.set(true);
        return Ok(false);
    }
    remove()?;
    Ok(true)
}

#[cfg(test)]
pub(super) mod tests;
