// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise shared page-scrolling gestures through the original native installer window.
use super::*;
/// Native arrow and thumb messages must reach the application; closing during capture releases it.
#[test]
fn native_installer_scroll_uses_shared_gestures_and_releases_drag_on_close() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::native_installer_scroll_uses_shared_gestures_and_releases_drag_on_close"
        ),
        qualify_scroll,
    )
}
fn qualify_scroll() -> TestResult {
    let mut fixture = Fixture::new(false)?;
    fixture.exercise_scroll = true;
    run(
        super::super::Window {
            title: "Native installer loop qualification".into(),
            visible: true,
            close: ControlId::new(1)?,
        },
        &mut fixture,
    )?;
    assert_eq!(fixture.count, 3);
    assert_eq!(fixture.scroll, 100.0);
    assert!(fixture.finished);
    Ok(())
}
impl Fixture {
    /// Drive real messages only to this private-desktop fixture's retained parent.
    pub(super) fn scroll_gesture(&self, hwnd: HWND) -> TestResult {
        use ::windows::Win32::{
            Foundation::{LPARAM, WPARAM},
            UI::{Input::KeyboardAndMouse::GetCapture, WindowsAndMessaging::*},
        };
        use sugarsubstitute_ui::windows::scrollbar::{self, Model, Part};
        let scale = self.shape.ok_or("Scroll fixture shape absent")?.scale();
        let model = Model {
            appearance: sugarsubstitute_ui::windows::scrollbar::Appearance::Gutter,
            bounds: RECT {
                left: (100.0 * scale).round() as i32,
                top: (120.0 * scale).round() as i32,
                right: (1080.0 * scale).round() as i32,
                bottom: (520.0 * scale).round() as i32,
            },
            scale,
            minimum: 0,
            maximum: 6400,
            page: 25600,
            position: (self.scroll * 64.0).round() as i32,
            dark: true,
            enabled: true,
            hover: 0,
            pressed: Part::None,
        };
        let geometry = scrollbar::geometry(&model)?;
        let packed = |x: i32, y: i32| LPARAM(((y as u16 as u32) << 16 | x as u16 as u32) as isize);
        let x = (geometry.thumb.left + geometry.thumb.right) / 2;
        if self.count == 1 {
            let y = (geometry.increment.top + geometry.increment.bottom) / 2;
            // SAFETY: Synchronous pointer messages target only this original private desktop window.
            unsafe {
                SendMessageW(hwnd, WM_KEYDOWN, Some(WPARAM(0x24)), Some(LPARAM(1)));
                SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(packed(x, y)));
                SendMessageW(hwnd, WM_LBUTTONUP, Some(WPARAM(0)), Some(packed(x, y)));
            }
        } else if self.count == 2 {
            assert_eq!(self.scroll, 20.0);
            let y = (geometry.thumb.top + geometry.thumb.bottom) / 2;
            // SAFETY: The shared original scroller owns this gesture; no global input is synthesized.
            unsafe {
                SendMessageW(hwnd, WM_LBUTTONDOWN, Some(WPARAM(1)), Some(packed(x, y)));
                assert_eq!(GetCapture(), hwnd);
                SendMessageW(
                    hwnd,
                    WM_MOUSEMOVE,
                    Some(WPARAM(1)),
                    Some(packed(x, model.bounds.bottom + 20)),
                );
            }
        } else {
            assert_eq!(self.scroll, 100.0);
            // SAFETY: Observe the original drag retained until native close settles its input owner.
            assert_eq!(unsafe { GetCapture() }, hwnd);
        }
        Ok(())
    }
}
