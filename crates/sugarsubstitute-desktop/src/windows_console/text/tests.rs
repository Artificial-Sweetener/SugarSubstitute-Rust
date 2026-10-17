// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify native console behavior within one actual outer UI STA lifetime.
use super::*;
use crate::{
    console::index::tests::{TestResult, index, source},
    windows_ui_apartment::UiApartment,
};
use windows::Win32::UI::WindowsAndMessaging::ES_READONLY;
#[path = "native_fixture.rs"]
mod native_fixture;
use native_fixture::{Hidden, MtaUsage, apartment_fact};

/// One actual UI lifetime covers implicit-MTA admission and every original native text scenario.
/// Any scenario error or panic retains both outer native obligations instead of claiming teardown.
#[test]
fn actual_implicit_mta_sta_lifetime_preserves_native_console_text_scroll_and_clamping() -> TestResult
{
    use windows::Win32::System::Com::{
        APTTYPE_MAINSTA, APTTYPE_MTA, APTTYPE_STA, APTTYPEQUALIFIER_IMPLICIT_MTA,
        APTTYPEQUALIFIER_NONE,
    };
    let mut mta = MtaUsage::new()?;
    let implicit = (APTTYPE_MTA, APTTYPEQUALIFIER_IMPLICIT_MTA);
    let initial = apartment_fact("ui_apartment_fixture_initial_query")?;
    if initial != implicit {
        return Err(DesktopError::message(
            "ui_apartment_fixture_initial_query",
            format!("expected implicit MTA, observed {initial:?}"),
        )
        .into());
    }
    let mut apartment = UiApartment::enter()?;
    let initialized = apartment_fact("ui_apartment_fixture_sta_query")?;
    // This independent native transition proves acquisition rather than borrowing an existing STA.
    let acquired = initialized.0 != initial.0;
    assert!(acquired);
    assert!(initialized.0 == APTTYPE_STA || initialized.0 == APTTYPE_MAINSTA);
    assert_eq!(initialized.1, APTTYPEQUALIFIER_NONE);

    actual_readonly_native_document_preserves_unicode_nul_styles_and_selected_character_space(
        &apartment,
    )?;
    actual_hidden_native_console_accepts_absent_scrollbar_and_retains_real_positions(&apartment)?;
    actual_native_selection_clamping_reports_requested_and_observed_ranges(&apartment)?;

    // SAFETY: Every scenario returned only after exact child, parent and module checked retirement.
    unsafe { apartment.release_after_teardown() }?;
    let restored = apartment_fact("ui_apartment_fixture_restored_query")?;
    mta.close()?;
    assert_eq!(restored, implicit);
    Ok(())
}

/// Preserve exact Unicode, embedded NUL, style and surrogate selection observations.
fn actual_readonly_native_document_preserves_unicode_nul_styles_and_selected_character_space(
    apartment: &UiApartment,
) -> TestResult {
    let (document, _frame, _released) =
        index(&source(1, &[(1, "a😀日本\0"), (2, "e\u{301} 끝")])?)?;
    let mut owner = Hidden::new(document, apartment)?;
    let hwnd = owner.hwnd.ok_or("hidden native peer absent")?;
    let mut port = NativeText { hwnd, dark: true };
    port.write(owner.document.as_ref().ok_or("original document absent")?)?;
    port.selection(1, 3)?;
    let selection = port.control().selection();
    assert_eq!(selection.cpMin, 1);
    assert_eq!(selection.cpMax, 3);
    let format = port.control().selected_format();
    assert!(format.Base.dwEffects.contains(CFE_BOLD));
    // SAFETY: Passive style observation of this exact hidden test-thread control.
    let style = unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::GWL_STYLE,
        )
    };
    assert_ne!(style as u32 & ES_READONLY as u32, 0);
    owner.close()?;
    Ok(())
}

/// A hidden empty control has no scroll bar; populated scrolling keeps native selection.
fn actual_hidden_native_console_accepts_absent_scrollbar_and_retains_real_positions(
    apartment: &UiApartment,
) -> TestResult {
    let empty_source = source(1, &[])?;
    let (empty, _first_frame, _first_release) = index(&empty_source)?;
    let mut owner = Hidden::new(empty, apartment)?;
    let hwnd = owner.hwnd.ok_or("hidden native peer absent")?;
    let mut port = NativeText { hwnd, dark: true };
    port.write(owner.document.as_ref().ok_or("original empty document absent")?)?;
    let empty = port.observe()?;
    assert_eq!((empty.start, empty.end, empty.first), (0, 0, 0));
    assert!(empty.at_bottom, "{}", {
        let mut scroll = SCROLLINFO {
            cbSize: size_of::<SCROLLINFO>() as u32,
            fMask: SIF_ALL,
            ..Default::default()
        };
        let mut client = windows::Win32::Foundation::RECT::default();
        let mut window = windows::Win32::Foundation::RECT::default();
        // SAFETY: Fresh failure-only observations of the exact retained hidden child.
        let (scroll_result, client_result, window_result, style) = unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                GWL_STYLE, GetClientRect, GetWindowLongPtrW, GetWindowRect,
            };
            (
                GetScrollInfo(hwnd, SB_VERT, &raw mut scroll),
                GetClientRect(hwnd, &raw mut client),
                GetWindowRect(hwnd, &raw mut window),
                GetWindowLongPtrW(hwnd, GWL_STYLE) as u32,
            )
        };
        format!(
            "empty scroll: length={}, scroll_error={:?}, range=({}, {}), page={}, pos={}, track={}, client_error={:?}, client=({}, {}, {}, {}), window_error={:?}, window=({}, {}, {}, {}), style=0x{style:08x}",
            port.control().length(),
            scroll_result.err().map(|cause| cause.code().0),
            scroll.nMin,
            scroll.nMax,
            scroll.nPage,
            scroll.nPos,
            scroll.nTrackPos,
            client_result.err().map(|cause| cause.code().0),
            client.left,
            client.top,
            client.right,
            client.bottom,
            window_result.err().map(|cause| cause.code().0),
            window.left,
            window.top,
            window.right,
            window.bottom,
        )
    });
    let lines: Vec<_> = (1..=100).map(|id| (id, "actual 日本語😀")).collect();
    let populated_source = source(2, &lines)?;
    let (populated, _next_frame, _next_release) = index(&populated_source)?;
    let original = owner.document.replace(populated);
    port.write(owner.document.as_ref().ok_or("populated document absent")?)?;
    drop(original);
    port.selection(2, 7)?;
    port.control().scroll_lines(8)?;
    let observed = port.observe()?;
    assert_eq!((observed.start, observed.end), (2, 7));
    assert!(observed.first > 0);
    assert!(!observed.at_bottom);
    owner.close()?;
    Ok(())
}

/// Native out-of-document clamping stays a failed exact selection with bounded evidence.
fn actual_native_selection_clamping_reports_requested_and_observed_ranges(
    apartment: &UiApartment,
) -> TestResult {
    let (document, _frame, _released) = index(&source(1, &[(1, "abc")])?)?;
    let units = document.units as i32;
    let mut owner = Hidden::new(document, apartment)?;
    let hwnd = owner.hwnd.ok_or("hidden native peer absent")?;
    let mut port = NativeText { hwnd, dark: true };
    port.write(owner.document.as_ref().ok_or("original document absent")?)?;
    let requested = i32::MAX;
    let cause = match port.selection(0, requested) {
        Err(cause) => cause,
        Ok(()) => return Err("native clamping was accepted as an exact selection".into()),
    };
    let actual = port.control().selection();
    assert_eq!(actual.cpMin, 0);
    // Rich-text controls retain one final paragraph marker beyond the copied text.
    assert!((0..=units + 1).contains(&actual.cpMax));
    assert!(actual.cpMax < requested);
    let diagnostic = cause.to_string();
    assert!(diagnostic.starts_with("console_selection_restore: "));
    assert!(diagnostic.contains(&format!("requested (0, {requested})")));
    assert!(diagnostic.contains(&format!("observed ({}, {})", actual.cpMin, actual.cpMax)));
    assert!(diagnostic.contains("native result "));
    port.selection(1, 2)?;
    let valid = port.control().selection();
    assert_eq!((valid.cpMin, valid.cpMax), (1, 2));
    owner.close()?;
    Ok(())
}
