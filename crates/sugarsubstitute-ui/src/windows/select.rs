// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the prepared Fluent selector's native font and full clickable height.
use super::NativeUiError;
use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT, WPARAM},
    Graphics::Gdi::HFONT,
    UI::WindowsAndMessaging::{
        CB_ERR, CB_GETITEMHEIGHT, CB_SETITEMHEIGHT, GetWindowRect, SendMessageW, WM_GETFONT,
        WM_SETFONT,
    },
};

/// Apply shared typography and match the actual combo peer to its prepared physical height.
/// Call on the owning UI thread after positioning; retain both old/new fonts until all peers switch.
/// # Errors
/// Rejects invalid height/font and preserves a rejected native assignment or geometry adjustment.
pub fn configure(hwnd: HWND, font: HFONT, height: i32) -> Result<(), NativeUiError> {
    if font.is_invalid() || !(16..=512).contains(&height) {
        return Err(rejected("native_select_admission"));
    }
    // SAFETY: The desktop caller retains this affine original selector and both font generations.
    unsafe {
        if SendMessageW(hwnd, WM_GETFONT, None, None).0 != font.0 as isize {
            SendMessageW(hwnd, WM_SETFONT, Some(WPARAM(font.0 as usize)), Some(LPARAM(0)));
            if SendMessageW(hwnd, WM_GETFONT, None, None).0 != font.0 as isize {
                return Err(rejected("native_select_font"));
            }
        }
    }
    let mut actual = RECT::default();
    // SAFETY: Query only the retained original peer's current physical bounds.
    unsafe { GetWindowRect(hwnd, &mut actual) }
        .map_err(|e| NativeUiError::new("native_select_bounds", e))?;
    let difference = height - (actual.bottom - actual.top);
    if difference != 0 {
        // SAFETY: Index -1 identifies the selection field, independently of popup rows and selection.
        let adjusted = unsafe {
            let item = SendMessageW(hwnd, CB_GETITEMHEIGHT, Some(WPARAM(usize::MAX)), None).0;
            if item == CB_ERR as isize {
                return Err(rejected("native_select_item_height"));
            }
            let wanted = item + difference as isize;
            if !(1..=512).contains(&wanted) {
                return Err(rejected("native_select_item_height"));
            }
            SendMessageW(hwnd, CB_SETITEMHEIGHT, Some(WPARAM(usize::MAX)), Some(LPARAM(wanted)))
        };
        if adjusted.0 == CB_ERR as isize {
            return Err(rejected("native_select_height"));
        }
        // SAFETY: Prove the actual input peer now covers the authored closed-control bounds.
        unsafe { GetWindowRect(hwnd, &mut actual) }
            .map_err(|e| NativeUiError::new("native_select_height", e))?;
        if actual.bottom - actual.top != height {
            return Err(rejected("native_select_height"));
        }
    }
    Ok(())
}
fn rejected(operation: &'static str) -> NativeUiError {
    NativeUiError::message(operation, "Native selector contract rejected")
}

/// Close an original selector before it becomes hidden or disabled; cancellation remains native.
/// # Errors
/// Reports a popup that remained expanded after its checked close request.
pub fn close(hwnd: HWND) -> Result<(), NativeUiError> {
    use windows::Win32::UI::WindowsAndMessaging::{CB_GETDROPPEDSTATE, CB_SHOWDROPDOWN};
    // SAFETY: Caller owns this affine selector and its original popup throughout synchronous close.
    let expanded = unsafe {
        SendMessageW(hwnd, CB_SHOWDROPDOWN, Some(WPARAM(0)), None);
        SendMessageW(hwnd, CB_GETDROPPEDSTATE, None, None).0
    };
    if expanded != 0 {
        return Err(rejected("native_select_close"));
    }
    Ok(())
}

/// Resolve the original OS popup belonging to one retained selector on its owning UI thread.
/// # Errors
/// Preserves a failed native peer query; this never searches for unrelated windows.
pub fn popup(hwnd: HWND) -> Result<HWND, NativeUiError> {
    use windows::Win32::UI::Controls::{COMBOBOXINFO, GetComboBoxInfo};
    let mut info = COMBOBOXINFO { cbSize: size_of::<COMBOBOXINFO>() as u32, ..Default::default() };
    // SAFETY: Caller retains the original affine combo throughout this synchronous query.
    unsafe { GetComboBoxInfo(hwnd, &mut info) }
        .map_err(|cause| NativeUiError::new("native_select_popup", cause))?;
    Ok(info.hwndList)
}

/// Apply the Fluent dropdown palette to an original popup's synchronous color request.
/// The returned DC brush is system-owned; it must not be deleted or retained beyond this paint.
/// OS selection, accessibility, keyboard input and scrolling remain with the original list.
/// # Errors
/// Rejects an invalid paint context or failed native color assignment.
pub fn popup_colors(
    dc: windows::Win32::Graphics::Gdi::HDC,
) -> Result<windows::Win32::Graphics::Gdi::HBRUSH, NativeUiError> {
    use windows::Win32::{
        Foundation::COLORREF,
        Graphics::Gdi::{
            DC_BRUSH, GetStockObject, HBRUSH, SetBkColor, SetDCBrushColor, SetTextColor,
        },
    };
    if dc.is_invalid() {
        return Err(rejected("native_select_palette"));
    }
    let [red, green, blue, _] = crate::widgets::TextTone::Primary.color();
    let foreground = COLORREF(u32::from(red) | (u32::from(green) << 8) | (u32::from(blue) << 16));
    let background = COLORREF(0x292929);
    // SAFETY: The parent lends this original list's paint DC only during WM_CTLCOLORLISTBOX.
    let (text, fill, brush_color, brush) = unsafe {
        (
            SetTextColor(dc, foreground),
            SetBkColor(dc, background),
            SetDCBrushColor(dc, background),
            GetStockObject(DC_BRUSH),
        )
    };
    if [text, fill, brush_color].iter().any(|color| color.0 == u32::MAX) || brush.is_invalid() {
        return Err(rejected("native_select_palette"));
    }
    Ok(HBRUSH(brush.0))
}
