// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Queue caption movement through the owning window's native procedure.
//!
//! The desktop UI thread owns the borrowed HWND and retains its Tao window until
//! its native loop ends. The queued system command carries no borrowed data or
//! screen-coordinate payload. Windows owns the modal movement
//! loop and pumps native messages through the existing window procedure.

use crate::DesktopError;
use std::ffi::c_void;
use tao::{platform::windows::WindowExtWindows, window::Window};
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, GetCapture, ReleaseCapture, VK_LBUTTON},
        WindowsAndMessaging::{HTCAPTION, PostMessageW, SC_MOVE, WM_SYSCOMMAND},
    },
};

/// Begin native pointer movement of this owned surface.
///
/// Call only on the creating UI thread after content classifies a caption press.
/// The native loop retains the window while the queued command is consumed.
/// Errors preserve capture and queue causes; another window's capture is retained.
pub(crate) fn begin(window: &Window) -> Result<(), DesktopError> {
    let hwnd = HWND(window.hwnd() as *mut c_void);
    // SAFETY: This native query reads the current physical button state without changing capture.
    let left_button_down = unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON.0)) } < 0;
    // SAFETY: Runtime owns this HWND on its creating UI thread until native loop teardown.
    let result = unsafe { queue_system_move(hwnd) };
    tracing::info!(
        operation = "native_caption_drag_queued",
        left_button_down,
        queued = result.is_ok(),
        "Observed native caption move queue result"
    );
    result
}

/// Release only this window's capture and queue its native pointer-move command.
///
/// # Safety
/// The caller owns any valid HWND on its creating UI thread and retains the
/// window through command consumption or native destruction. Invalid handles are
/// rejected before native capture is queried. No pointer data is posted; the
/// native event loop must continue pumping the window's messages.
unsafe fn queue_system_move(hwnd: HWND) -> Result<(), DesktopError> {
    if hwnd.is_invalid() {
        return Err(DesktopError::message("begin_caption_drag", "owned window handle is invalid"));
    }
    // SAFETY: GetCapture observes only this UI thread. ReleaseCapture is permitted
    // only for our HWND, preserving another window's capture and native lifecycle.
    unsafe {
        let capture = GetCapture();
        if capture != HWND::default() && capture != hwnd {
            return Err(DesktopError::message(
                "begin_caption_drag",
                "another window on this UI thread owns mouse capture",
            ));
        }
        if capture == hwnd {
            ReleaseCapture()
                .map_err(|error| DesktopError::new("release_caption_drag_capture", error))?;
        }
        // The pointer-drag form of SC_MOVE uses the caption hit-test in its low
        // command bits. Windows obtains its own cursor position during movement.
        PostMessageW(Some(hwnd), WM_SYSCOMMAND, WPARAM((SC_MOVE | HTCAPTION) as usize), LPARAM(0))
            .map_err(|error| DesktopError::new("queue_caption_drag", error))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use windows::{
        Win32::UI::{
            Input::KeyboardAndMouse::SetCapture,
            WindowsAndMessaging::{
                CreateWindowExW, DestroyWindow, MSG, PM_REMOVE, PeekMessageW, WINDOW_EX_STYLE,
                WINDOW_STYLE,
            },
        },
        core::w,
    };

    /// Own an invisible test HWND without requesting focus or movement.
    struct HiddenWindow(HWND);

    impl HiddenWindow {
        /// Create a thread-owned invisible native fixture.
        fn new() -> Result<Self, Box<dyn Error>> {
            // SAFETY: STATIC is an OS class; this thread exclusively owns the hidden HWND.
            Ok(Self(unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("STATIC"),
                    w!("System move queue fixture"),
                    WINDOW_STYLE::default(),
                    0,
                    0,
                    1,
                    1,
                    None,
                    None,
                    None,
                    None,
                )?
            }))
        }

        /// Remove only this fixture's system command without dispatching a native move loop.
        fn take_move(&self) -> Option<MSG> {
            let mut message = MSG::default();
            // SAFETY: This test thread owns the retained HWND and output structure.
            unsafe {
                PeekMessageW(
                    &raw mut message,
                    Some(self.0),
                    WM_SYSCOMMAND,
                    WM_SYSCOMMAND,
                    PM_REMOVE,
                )
                .as_bool()
                .then_some(message)
            }
        }
    }

    impl Drop for HiddenWindow {
        /// Destroy this thread's fixture even when an assertion fails.
        fn drop(&mut self) {
            // SAFETY: The fixture is owned by this test thread until synchronous destruction.
            if let Err(error) = unsafe { DestroyWindow(self.0) } {
                tracing::error!(operation = "destroy_caption_drag_fixture", cause = %error, "Native fixture destruction failed");
            }
        }
    }

    /// Observe the exact native pointer-move command after its producer returns.
    #[test]
    fn native_system_move_queue_carries_pointer_drag_command() -> Result<(), Box<dyn Error>> {
        let window = HiddenWindow::new()?;
        // SAFETY: This hidden HWND belongs to this test thread and is retained until teardown.
        unsafe { queue_system_move(window.0)? };
        let message = window.take_move().ok_or("native system move command was not queued")?;
        assert_eq!(message.hwnd, window.0);
        assert_eq!(message.message, WM_SYSCOMMAND);
        assert_eq!(message.wParam, WPARAM((SC_MOVE | HTCAPTION) as usize));
        assert_eq!(message.lParam, LPARAM(0));
        assert!(window.take_move().is_none());
        Ok(())
    }

    /// Preserve another fixture's capture and reject movement without posting a command.
    #[test]
    fn native_system_move_rejects_foreign_capture() -> Result<(), Box<dyn Error>> {
        let window = HiddenWindow::new()?;
        let foreign = HiddenWindow::new()?;
        // SAFETY: Both hidden HWNDs belong to this test thread; no window is shown or focused.
        unsafe {
            assert_eq!(GetCapture(), HWND::default());
            SetCapture(foreign.0);
            assert_eq!(GetCapture(), foreign.0);
            assert!(queue_system_move(window.0).is_err());
            assert_eq!(GetCapture(), foreign.0);
        }
        assert!(window.take_move().is_none());
        Ok(())
    }

    /// Release the owned fixture's actual capture before queueing movement.
    #[test]
    fn native_system_move_releases_only_owned_capture() -> Result<(), Box<dyn Error>> {
        let window = HiddenWindow::new()?;
        // SAFETY: The hidden HWND belongs to this thread and remains alive through the query.
        unsafe {
            assert_eq!(GetCapture(), HWND::default());
            SetCapture(window.0);
            assert_eq!(GetCapture(), window.0);
            queue_system_move(window.0)?;
            assert_eq!(GetCapture(), HWND::default());
        }
        assert!(window.take_move().is_some());
        Ok(())
    }

    /// Invalid destinations must not release existing capture or post a thread command.
    #[test]
    fn native_system_move_rejects_invalid_destination() -> Result<(), Box<dyn Error>> {
        let capture = HiddenWindow::new()?;
        let mut message = MSG::default();
        // SAFETY: This thread owns the hidden capture fixture; null handles are explicitly rejected.
        unsafe {
            assert_eq!(GetCapture(), HWND::default());
            assert!(queue_system_move(HWND::default()).is_err());
            assert!(
                !PeekMessageW(&raw mut message, None, WM_SYSCOMMAND, WM_SYSCOMMAND, PM_REMOVE)
                    .as_bool()
            );
            SetCapture(capture.0);
            assert_eq!(GetCapture(), capture.0);
            assert!(queue_system_move(HWND::default()).is_err());
            assert_eq!(GetCapture(), capture.0);
        }
        assert!(capture.take_move().is_none());
        Ok(())
    }
}
