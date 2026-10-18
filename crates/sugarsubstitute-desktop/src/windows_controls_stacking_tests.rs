// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise native sibling stacking without displaying a window or capturing the desktop.
use super::*;
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, GetTopWindow, GetWindowRect};

struct Fixture(Vec<HWND>);
impl Drop for Fixture {
    fn drop(&mut self) {
        for hwnd in self.0.iter().rev() {
            // SAFETY: The fixture creates and retains each exact hidden HWND on this test thread.
            if let Err(cause) = unsafe { DestroyWindow(*hwnd) } {
                tracing::error!(operation="retire_caption_stacking_fixture", cause=%cause,
                    "Native stacking fixture retirement failed");
            }
        }
    }
}
impl Fixture {
    /// Retain a created hidden parent or native child before any later assertion can fail.
    fn create(&mut self, parent: Option<HWND>) -> Result<HWND, DesktopError> {
        // SAFETY: The built-in STATIC class requires no callback lifetime. All windows stay hidden.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!(""),
                if parent.is_some() { WS_CHILD } else { WINDOW_STYLE::default() },
                0,
                0,
                138,
                34,
                parent,
                None,
                None,
                None,
            )
        }
        .map_err(|cause| DesktopError::new("create_caption_stacking_fixture", cause))?;
        self.0.push(hwnd);
        Ok(hwnd)
    }
}

#[test]
fn original_native_caption_stays_above_a_later_browser_container_without_geometry_change()
-> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = Fixture(Vec::new());
    let parent = fixture.create(None)?;
    let caption = fixture.create(Some(parent))?;
    let browser = fixture.create(Some(parent))?;
    raise(browser)?;
    // SAFETY: This thread owns each still-hidden native parent and child.
    assert_eq!(unsafe { GetTopWindow(Some(parent)) }?, browser);
    let mut before = RECT::default();
    let mut after = RECT::default();
    // SAFETY: Exact original child and local output remain live during the getter.
    unsafe { GetWindowRect(caption, &raw mut before) }?;
    raise(caption)?;
    // SAFETY: Original hidden siblings remain retained without event-loop recreation.
    assert_eq!(unsafe { GetTopWindow(Some(parent)) }?, caption);
    // SAFETY: Original hidden child and output remain live after the stacking effect.
    unsafe { GetWindowRect(caption, &raw mut after) }?;
    assert_eq!(before, after);
    let mut client = RECT::default();
    // SAFETY: Read the exact child client bounds without mapping or activating it.
    unsafe { GetClientRect(caption, &raw mut client) }?;
    assert_eq!((client.right, client.bottom), (138, 34));
    Ok(())
}
