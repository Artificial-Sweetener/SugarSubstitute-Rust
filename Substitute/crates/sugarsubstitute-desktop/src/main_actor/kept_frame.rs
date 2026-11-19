// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep a reloading window's frame on screen until the window that replaces it shows its own.
//!
//! A frontend reload retires the whole main lifetime and builds another, and the browser's
//! profile is released in between, which takes seconds. So that the user never looks at a
//! screen without the application, the retiring lifetime hands its bare frame (the window, its
//! material and its caption buttons, with no browser left in it) to its successor, which
//! retires it the moment its own frame is on screen. The frame answers nothing meanwhile: its
//! caption buttons name the retired lifetime, which no longer exists.

use super::*;
use tao::platform::windows::WindowExtWindows;
use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::DestroyWindow};

/// A retired main lifetime's window, material and caption buttons, still on screen.
pub(crate) struct KeptFrame {
    window: Option<Rc<Window>>,
    presenter: Option<Presenter>,
    controls: Option<Controls>,
}

impl KeptFrame {
    /// Take custody of a lifetime's frame after its browser has retired.
    pub(super) fn new(
        window: Rc<Window>,
        presenter: Option<Presenter>,
        controls: Option<Controls>,
    ) -> Self {
        Self { window: Some(window), presenter, controls }
    }

    /// Retire one native owner: the caption buttons, then the material, then the window.
    /// True once nothing is left.
    ///
    /// # Errors
    /// Returns the native failure; custody stays here for another attempt.
    pub(super) fn retire_step(&mut self) -> Result<bool, DesktopError> {
        if let Some(controls) = &mut self.controls {
            controls.shutdown()?;
            if controls.retired() {
                self.controls = None;
            }
            return Ok(false);
        }
        if let Some(presenter) = &mut self.presenter {
            presenter.shutdown()?;
            self.presenter = None;
            return Ok(false);
        }
        if let Some(window) = &self.window {
            let hwnd = HWND(window.hwnd() as *mut std::ffi::c_void);
            // SAFETY: Every child owner has retired; this creating thread retains the exact
            // window until its destruction succeeds.
            unsafe { DestroyWindow(hwnd) }
                .map_err(|cause| DesktopError::new("destroy_kept_main_frame", cause))?;
            self.window = None;
        }
        Ok(true)
    }
}

impl Drop for KeptFrame {
    /// Retain unresolved native references rather than release them off their retirement path.
    fn drop(&mut self) {
        if self.window.is_some() || self.presenter.is_some() || self.controls.is_some() {
            std::mem::forget(self.window.take());
            std::mem::forget(self.presenter.take());
            std::mem::forget(self.controls.take());
            tracing::error!(
                operation = "kept_main_frame_unresolved_drop",
                "Retaining a reloading window's frame until process termination"
            );
        }
    }
}
