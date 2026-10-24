// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Verify localized title publication on the actual creating-thread HWND.
use std::error::Error;
use tao::{platform::windows::WindowExtWindows, window::Window};
use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::GetWindowTextW};

/// Bound the native readback and compare UTF-16 units without normalizing Unicode.
/// The caller retains the same live native window on its creating UI thread.
pub(super) fn verify(window: &Window, expected: &str) -> Result<(), Box<dyn Error>> {
    let mut units = [0; 512];
    // SAFETY: The creating-thread window and exclusive bounded output slice remain live for this synchronous call.
    let length = unsafe { GetWindowTextW(HWND(window.hwnd() as *mut _), &mut units) };
    if length <= 0 {
        return Err(windows::core::Error::from_thread().into());
    }
    let actual = usize::try_from(length)
        .ok()
        .and_then(|length| units.get(..length))
        .ok_or("Native installer title returned an invalid length")?;
    if !actual.iter().copied().eq(expected.encode_utf16()) {
        return Err("Native installer title differs from the selected localized copy".into());
    }
    tracing::info!(
        operation = "installer_title_localized",
        title = expected,
        "Localized native installer title verified"
    );
    Ok(())
}
