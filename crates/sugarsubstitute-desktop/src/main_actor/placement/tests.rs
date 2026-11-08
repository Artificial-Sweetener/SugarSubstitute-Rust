// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify Win32 workspace conversion without creating or changing native windows.
use super::*;
use std::error::Error;

mod native_cases;

#[test]
fn placement_workspace_rectangle_adds_only_top_left_work_area_inset() -> Result<(), Box<dyn Error>>
{
    let monitor = MONITORINFO {
        rcMonitor: RECT { left: -1920, top: -1080, right: 0, bottom: 0 },
        rcWork: RECT { left: -1880, top: -1040, right: 0, bottom: 0 },
        ..Default::default()
    };
    let normal = RECT { left: -1800, top: -1000, right: -800, bottom: -300 };
    let screen = workspace_to_screen(normal, monitor)?;
    assert_eq!(screen, Rect { x: -1760.0, y: -960.0, width: 1000.0, height: 700.0 });
    Ok(())
}

#[test]
fn placement_bottom_taskbar_does_not_shift_screen_origin() -> Result<(), Box<dyn Error>> {
    let monitor = MONITORINFO {
        rcMonitor: RECT { left: 0, top: 0, right: 1920, bottom: 1080 },
        rcWork: RECT { left: 0, top: 0, right: 1920, bottom: 1040 },
        ..Default::default()
    };
    let normal = RECT { left: 100, top: 120, right: 1100, bottom: 820 };
    assert_eq!(
        workspace_to_screen(normal, monitor)?,
        Rect { x: 100.0, y: 120.0, width: 1000.0, height: 700.0 }
    );
    Ok(())
}

#[test]
fn placement_invalid_normal_restore_rectangle_is_rejected() {
    assert!(workspace_to_screen(RECT::default(), MONITORINFO::default()).is_err());
}

#[test]
fn placement_minimized_capture_retains_previous_maximized_or_normal_intent() {
    let mut state = WINDOWPLACEMENT { showCmd: SW_SHOWMAXIMIZED.0 as u32, ..Default::default() };
    assert!(maximized_intent(&state, false));
    state.showCmd = 2;
    assert!(!maximized_intent(&state, true));
    state.flags = WPF_RESTORETOMAXIMIZED;
    assert!(maximized_intent(&state, true));
    assert!(!maximized_intent(&state, false));
}
