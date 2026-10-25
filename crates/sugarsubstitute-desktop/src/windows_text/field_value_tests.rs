// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real field reconciliation must not reset newer native edits or the user's selection/history.
use super::{Engine, TestResult};
use windows::{
    Win32::{
        Foundation::{LPARAM, WPARAM},
        UI::{
            Controls::{EM_CANUNDO, EM_GETSEL, EM_REPLACESEL, EM_SETSEL, EM_UNDO},
            WindowsAndMessaging::{SendMessageW, WM_SETTEXT},
        },
    },
    core::w,
};

pub(super) fn qualify(engine: &Engine) -> TestResult {
    let initial = engine.field_value()?;
    let path = "E:\\用户\\🐈";
    assert!(engine.replace_field(initial.revision, path)?);
    let displayed = engine.field_value()?;
    assert_eq!(displayed.text.as_deref(), Some(path));
    assert!(displayed.revision > initial.revision);
    engine.message(EM_SETSEL, WPARAM(2), LPARAM(5))?;
    let selection = engine.message(EM_GETSEL, WPARAM(0), LPARAM(0))?;
    assert!(engine.replace_field(displayed.revision, path)?);
    assert_eq!(engine.field_value()?.revision, displayed.revision);
    assert_eq!(engine.message(EM_GETSEL, WPARAM(0), LPARAM(0))?, selection);

    let end = path.encode_utf16().count();
    engine.message(EM_SETSEL, WPARAM(end), LPARAM(end as isize))?;
    engine.message(EM_REPLACESEL, WPARAM(1), LPARAM(w!("-edited").as_ptr() as isize))?;
    let edited = engine.field_value()?;
    assert_eq!(edited.text, Some(format!("{path}-edited")));
    assert!(edited.revision > displayed.revision);
    engine.message(EM_SETSEL, WPARAM(1), LPARAM(4))?;
    let selected = engine.message(EM_GETSEL, WPARAM(0), LPARAM(0))?;
    assert_eq!(engine.message(EM_CANUNDO, WPARAM(0), LPARAM(0))?.0, 1);
    assert!(!engine.replace_field(displayed.revision, "stale render snapshot")?);
    assert_eq!(engine.field_value()?.text, edited.text);
    assert_eq!(engine.message(EM_GETSEL, WPARAM(0), LPARAM(0))?, selected);

    let unchanged: Vec<u16> =
        edited.text.as_deref().ok_or("Edit is incomplete")?.encode_utf16().chain([0]).collect();
    assert_eq!(
        // SAFETY: The original retained private-desktop child reads this live terminated string.
        unsafe {
            SendMessageW(
                engine.state.hwnd,
                WM_SETTEXT,
                None,
                Some(LPARAM(unchanged.as_ptr() as isize)),
            )
        }
        .0,
        1
    );
    assert_eq!(engine.field_value()?.revision, edited.revision);
    assert_eq!(engine.message(EM_GETSEL, WPARAM(0), LPARAM(0))?, selected);
    assert_eq!(engine.message(EM_CANUNDO, WPARAM(0), LPARAM(0))?.0, 1);
    engine.message(EM_UNDO, WPARAM(0), LPARAM(0))?;
    assert_eq!(engine.field_value()?.text.as_deref(), Some(path));

    for invalid in ["🙂".repeat(2049), "bad\npath".into(), "bad\rpath".into()] {
        let before = engine.field_value()?;
        assert!(engine.replace_field(before.revision, &invalid).is_err());
        let native: Vec<u16> = invalid.encode_utf16().chain([0]).collect();
        assert_eq!(
            // SAFETY: The original retained peer reads this live terminated invalid input.
            unsafe {
                SendMessageW(
                    engine.state.hwnd,
                    WM_SETTEXT,
                    None,
                    Some(LPARAM(native.as_ptr() as isize)),
                )
            }
            .0,
            0
        );
        let after = engine.field_value()?;
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.text, before.text);
        engine.check()?;
    }
    // SAFETY: The native WM_SETTEXT contract treats a null input as an empty replacement.
    assert_eq!(unsafe { SendMessageW(engine.state.hwnd, WM_SETTEXT, None, None) }.0, 1);
    assert_eq!(engine.field_value()?.text.as_deref(), Some(""));
    let partial = [0xd83du16, 0];
    engine.message(EM_REPLACESEL, WPARAM(1), LPARAM(partial.as_ptr() as isize))?;
    assert!(engine.field_value()?.text.is_none(), "partial UTF-16 must stay in the native editor");
    engine.check()?;
    let completion = [0xde00u16, 0];
    engine.message(EM_REPLACESEL, WPARAM(1), LPARAM(completion.as_ptr() as isize))?;
    assert_eq!(engine.field_value()?.text.as_deref(), Some("😀"));
    engine.close()?;
    assert!(engine.field_value().is_err());
    assert!(engine.replace_field(0, "late").is_err());
    Ok(())
}
