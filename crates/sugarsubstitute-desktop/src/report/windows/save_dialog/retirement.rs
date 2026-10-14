// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the actual disarmed native modal parameters until observed callback retirement.
use super::Dialog;
pub(in super::super) struct UnretiredDialog {
    dialog: Option<Dialog>,
}
#[cfg(test)]
#[path = "retirement/tests.rs"]
mod tests;
impl UnretiredDialog {
    pub(super) fn new(dialog: Dialog) -> Self {
        Self { dialog: Some(dialog) }
    }
    /// Called after native dispatch. Only actual destroy and checked timer retirement release storage.
    pub(in super::super) fn observe_retirement(&mut self) -> bool {
        let retired = self.dialog.as_ref().is_none_or(|dialog| dialog.hook.retired());
        if retired {
            self.dialog = None;
        }
        retired
    }
}
impl Drop for UnretiredDialog {
    fn drop(&mut self) {
        // Unresolved native borrows keep their actual parameters alive through process termination.
        if let Some(dialog) = self.dialog.take() {
            std::mem::forget(dialog);
        }
    }
}
