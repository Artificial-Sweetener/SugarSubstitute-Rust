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
        // SAFETY: Built-in STATIC/BUTTON classes require no callback lifetime. Parents stay hidden.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                if parent.is_some() { w!("BUTTON") } else { w!("STATIC") },
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

/// Exercise the actual shared Win32 effects against hidden, retained native BUTTON peers.
#[test]
fn shared_native_availability_keeps_geometry_stacking_and_hidden_parent_intact()
-> Result<(), Box<dyn std::error::Error>> {
    use sugarsubstitute_ui::windows::availability::{set_enabled, set_visible};
    use windows::Win32::UI::{
        Input::KeyboardAndMouse::{GetFocus, IsWindowEnabled},
        WindowsAndMessaging::{GWL_STYLE, GetWindowLongPtrW, IsWindowVisible},
    };
    let mut fixture = Fixture(Vec::new());
    let parent = fixture.create(None)?;
    let child = fixture.create(Some(parent))?;
    let sibling = fixture.create(Some(parent))?;
    raise(sibling)?;
    let mut original = RECT::default();
    // SAFETY: The fixture retains these hidden HWNDs and writable output on this test thread.
    unsafe { GetWindowRect(child, &raw mut original) }?;
    // SAFETY: Passive query of this test thread's focus; no input/focus is synthesized.
    let focus = unsafe { GetFocus() };
    for (visible, enabled) in [(true, false), (true, true), (false, false), (true, false)] {
        set_enabled(child, enabled)?;
        set_visible(child, visible)?;
        let mut actual = RECT::default();
        // SAFETY: All observations refer to the same retained fixture windows and local outputs.
        unsafe {
            assert_eq!(IsWindowEnabled(child).as_bool(), enabled);
            assert_eq!(GetWindowLongPtrW(child, GWL_STYLE) as u32 & WS_VISIBLE.0 != 0, visible);
            assert!(!IsWindowVisible(parent).as_bool());
            assert!(!IsWindowVisible(child).as_bool());
            assert_eq!(GetFocus(), focus);
            assert_eq!(GetTopWindow(Some(parent))?, sibling);
            GetWindowRect(child, &raw mut actual)?;
        }
        assert_eq!(actual, original);
    }
    assert!(set_visible(parent, true).is_err());
    assert!(set_enabled(parent, false).is_err());
    assert!(set_enabled(HWND::default(), true).is_err());
    assert!(set_visible(HWND::default(), true).is_err());
    Ok(())
}
