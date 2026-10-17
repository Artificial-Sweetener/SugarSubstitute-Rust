// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Traverse only this surface's retained, visible peers on the creating UI thread.
use super::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, SetFocus, VK_SHIFT};
/// Read actual UI-thread Shift state before recording native traversal direction.
pub(crate) fn shift_pressed() -> bool {
    // SAFETY: Passive UI-thread key-state observation; no global input is synthesized.
    unsafe { GetKeyState(i32::from(VK_SHIFT.0)) < 0 }
}
/// Set focus and verify actual resulting identity; a null previous-focus return can succeed.
pub(crate) fn focus_window(hwnd: HWND) -> Result<(), DesktopError> {
    // SAFETY: Caller supplies only an exact retained peer or parent on its creating thread.
    let result = unsafe { SetFocus(Some(hwnd)) };
    // SAFETY: Observe this same native thread's actual focus after synchronous reentry.
    if unsafe { GetFocus() } != hwnd {
        return Err(match result {
            Err(e) => DesktopError::new("splash_focus", e),
            Ok(_) => DesktopError::message("splash_focus", "native focus identity differs"),
        });
    }
    Ok(())
}
impl Controls {
    /// Tab and Shift+Tab wrap among current visible native peers without activation.
    pub(crate) fn focus_next(&self, backward: bool) -> Result<(), DesktopError> {
        if !self.creation.writable() || self.paint_mode != PaintMode::PreparedPixels {
            return Ok(());
        }
        let console = self.console.as_ref().and_then(console::Console::hwnd);
        // SAFETY: Passive query on the original UI thread; it grants no foreign HWND authority.
        let current = unsafe { GetFocus() };
        use crate::console::focus::{self, Target};
        let source = if console == Some(current) {
            Some(Target::Console)
        } else {
            self.creation
                .peers
                .iter()
                .map(|slot| &slot.value)
                .find(|peer| peer.hwnd == current && peer.snapshot.visible)
                .map(|peer| Target::Control(peer.snapshot.id))
        };
        let order = self.console.as_ref().and_then(console::Console::order_y);
        let target = focus::next(
            self.creation.peers.iter().map(|peer| peer.value.snapshot),
            order,
            source,
            backward,
        );
        let hwnd = match target {
            None => return Ok(()),
            Some(Target::Console) => console,
            Some(Target::Control(id)) => self
                .creation
                .peers
                .iter()
                .map(|slot| &slot.value)
                .find(|peer| peer.snapshot.id == id && peer.snapshot.visible)
                .map(|peer| peer.hwnd),
        }
        .ok_or_else(|| DesktopError::message("splash_focus", "retained focus target absent"))?;
        focus_window(hwnd)
    }
}
