// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep original native controls above a sibling browser container.

use super::*;
use windows::Win32::UI::WindowsAndMessaging::{HWND_TOP, IsWindow, SWP_NOMOVE, SWP_NOSIZE};

impl Controls {
    /// Raise exact retained Main peers after browser construction, showing, or layout.
    /// Every possibly reentrant native effect rechecks original callback custody.
    pub(crate) fn raise_main(&self) -> Result<(), DesktopError> {
        if !matches!(self.paint_mode, PaintMode::NativeButtons(_)) {
            return Err(DesktopError::message("raise_main_caption", "surface is not Main"));
        }
        for slot in &self.creation.peers {
            let peer = &slot.value;
            if !peer.snapshot.visible {
                continue;
            }
            self.original_peer(peer)?;
            raise(peer.hwnd)?;
            self.original_peer(peer)?;
        }
        Ok(())
    }

    /// Require the original parent and child subclass lifetimes before trusting a raw HWND.
    pub(super) fn original_peer(&self, peer: &Peer) -> Result<(), DesktopError> {
        if !self.creation.writable()
            || self
                .creation
                .parent
                .as_ref()
                .is_none_or(|owner| !owner.installed() || owner.destroyed())
            || peer
                .registration
                .as_ref()
                .is_none_or(|owner| !owner.installed() || owner.destroyed())
        {
            return Err(DesktopError::message("raise_main_caption", "original peer retired"));
        }
        // SAFETY: Only observe this retained child's identity on its creating UI thread.
        if !unsafe { IsWindow(Some(peer.hwnd)) }.as_bool() {
            return Err(DesktopError::message("raise_main_caption", "native peer retired"));
        }
        Ok(())
    }
}

/// Reorder a retained child without activating, moving, resizing, or recreating it.
pub(super) fn raise(hwnd: HWND) -> Result<(), DesktopError> {
    // SAFETY: The caller retains the original child on its creating UI thread.
    unsafe {
        SetWindowPos(hwnd, Some(HWND_TOP), 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE)
    }
    .map_err(|cause| DesktopError::new("raise_native_control", cause))
}

#[cfg(test)]
#[path = "windows_controls_stacking_tests.rs"]
mod tests;
