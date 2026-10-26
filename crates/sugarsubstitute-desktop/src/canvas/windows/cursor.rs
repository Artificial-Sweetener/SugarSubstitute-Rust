// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind the shared DPI-scaled hand masks to owned native cursor resources.
use super::*;
use ::windows::Win32::UI::{
    HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
    WindowsAndMessaging::{
        CreateCursor, DestroyCursor, GetCursor, HCURSOR, IDC_ARROW, LoadCursorW, SM_CXCURSOR,
        SetCursor,
    },
};
use sugarsubstitute_canvas::CursorIntent;
use sugarsubstitute_ui::cursor::{HandCursor, HandState};

/// Keep owned hand resources alive while callbacks retain their borrowed native handles.
pub(super) struct Cursors {
    dpi: u32,
    arrow: HCURSOR,
    open: Option<HCURSOR>,
    closed: Option<HCURSOR>,
}
impl Cursors {
    pub(super) fn new(window: HWND) -> Result<Self, Box<dyn Error>> {
        // SAFETY: The original HWND remains live on the calling UI thread.
        let dpi = unsafe { GetDpiForWindow(window) };
        if dpi == 0 {
            return Err("Native cursor DPI is unavailable".into());
        }
        // SAFETY: The shared system arrow has OS lifetime; metrics own no native resource.
        let arrow = unsafe { LoadCursorW(None, IDC_ARROW)? };
        // SAFETY: The already-validated DPI supplies a bounded system metric query.
        let width = unsafe { GetSystemMetricsForDpi(SM_CXCURSOR, dpi) };
        let width = u32::try_from(width)?.clamp(16, 128);
        let mut result = Self { dpi, arrow, open: None, closed: None };
        result.open = Some(create(HandState::Open, width)?);
        result.closed = Some(create(HandState::Closed, width)?);
        Ok(result)
    }
    pub(super) fn changed(&self, window: HWND) -> bool {
        // SAFETY: The desktop retains the same HWND through this synchronous query.
        self.dpi != unsafe { GetDpiForWindow(window) }
    }
    pub(super) fn get(&self, intent: CursorIntent) -> Result<HCURSOR, DesktopError> {
        match intent {
            CursorIntent::Arrow => Some(self.arrow),
            CursorIntent::OpenHand => self.open,
            CursorIntent::ClosedHand => self.closed,
        }
        .ok_or_else(|| DesktopError::message("canvas_cursor", "Native cursor has retired"))
    }
    pub(super) fn retire(&mut self) -> Result<(), DesktopError> {
        let open = destroy(&mut self.open, self.arrow);
        let closed = destroy(&mut self.closed, self.arrow);
        match (open, closed) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(cause), Ok(())) | (Ok(()), Err(cause)) => Err(cause),
            (Err(primary), Err(retirement)) => Err(DesktopError::new(
                "canvas_cursor_retirement",
                NativeFailures { primary: Box::new(primary), retirement: Box::new(retirement) },
            )),
        }
    }
}
impl Drop for Cursors {
    fn drop(&mut self) {
        if let Err(cause) = self.retire() {
            tracing::error!(operation="canvas_cursor_retirement", cause=%cause, "Native cursor retirement failed");
        }
    }
}
fn create(state: HandState, width: u32) -> Result<HCURSOR, Box<dyn Error>> {
    let mask = HandCursor::new(state, width)?;
    let hotspot = mask.hotspot();
    // SAFETY: Both complete word-aligned monochrome masks outlive this synchronous copying API.
    // The returned private cursor is destroyed exactly once by its same-thread resource owner.
    Ok(unsafe {
        CreateCursor(
            None,
            hotspot[0] as i32,
            hotspot[1] as i32,
            width as i32,
            width as i32,
            mask.and_bits().as_ptr().cast(),
            mask.xor_bits().as_ptr().cast(),
        )?
    })
}
fn destroy(cursor: &mut Option<HCURSOR>, arrow: HCURSOR) -> Result<(), DesktopError> {
    let Some(handle) = *cursor else {
        return Ok(());
    };
    // SAFETY: Replace only this owner's current private handle with its OS-owned shared arrow.
    // Other controls' cursor handles are left intact before the private allocation is released.
    if unsafe { GetCursor() } == handle {
        // SAFETY: Production owners retain the shared system arrow for the process lifetime.
        unsafe {
            SetCursor(Some(arrow));
        }
    }
    // SAFETY: This is an owned, non-shared resource; native callbacks have retired or use its replacement.
    unsafe { DestroyCursor(handle) }
        .map_err(|cause| DesktopError::new("canvas_cursor_retirement", cause))?;
    *cursor = None;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use ::windows::Win32::{
        Graphics::Gdi::{BITMAP, DeleteObject, GetObjectW, HGDIOBJ},
        UI::WindowsAndMessaging::{GetIconInfo, HICON, ICONINFO},
    };
    #[test]
    fn private_native_hands_copy_dpi_masks_and_keep_hotspots_until_checked_retirement()
    -> Result<(), Box<dyn Error>> {
        for width in [32, 40, 64, 128] {
            let mut owner = Cursors {
                dpi: 96,
                arrow: HCURSOR::default(),
                open: Some(create(HandState::Open, width)?),
                closed: Some(create(HandState::Closed, width)?),
            };
            for intent in [CursorIntent::OpenHand, CursorIntent::ClosedHand] {
                let mut info = ICONINFO::default();
                // SAFETY: The cursor remains owned across this synchronous metadata copy.
                unsafe {
                    GetIconInfo(HICON(owner.get(intent)?.0), &mut info)?;
                }
                let mut bitmap = BITMAP::default();
                // SAFETY: GetIconInfo returns private bitmap copies; local storage has the exact
                // declared BITMAP capacity and both copies are released before assertions.
                let copied = unsafe {
                    GetObjectW(
                        HGDIOBJ(info.hbmMask.0),
                        size_of::<BITMAP>() as i32,
                        Some((&raw mut bitmap).cast()),
                    )
                };
                // SAFETY: These are exactly this query's private bitmap copies, never cursor-owned handles.
                let mask_removed = unsafe { DeleteObject(HGDIOBJ(info.hbmMask.0)) }.as_bool();
                let color_removed = if info.hbmColor.is_invalid() {
                    true
                } else {
                    // SAFETY: A non-null color bitmap is another private copy from that same query.
                    unsafe { DeleteObject(HGDIOBJ(info.hbmColor.0)) }.as_bool()
                };
                assert!(mask_removed && color_removed);
                assert_eq!(copied as usize, size_of::<BITMAP>());
                assert!(!info.fIcon.as_bool());
                assert_eq!([info.xHotspot, info.yHotspot], [width * 12 / 32, width / 2]);
                assert_eq!([bitmap.bmWidth, bitmap.bmHeight], [width as i32, width as i32 * 2]);
            }
            owner.retire()?;
            assert!(owner.get(CursorIntent::OpenHand).is_err());
            assert!(owner.get(CursorIntent::ClosedHand).is_err());
            owner.retire()?;
        }
        Ok(())
    }
}
