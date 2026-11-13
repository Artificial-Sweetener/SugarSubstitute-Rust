// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Build the cursors a canvas drag-out shows, as CuteCanvas's Qt drag showed them.
//!
//! Qt's Windows drag source turns the drag pixmap into the cursor itself: the preview, fitted to
//! 15% of the pointer monitor's available area and placed 10 pixels right of and below the drop
//! effect's cursor, whose hotspot stays the pointer's. A target that accepts the copy shows Qt's
//! copy artwork; one that refuses it shows the system "unavailable" cursor's own pixels.

use super::*;
use crate::canvas::windows::cursor::{OwnedCursor, rgba_cursor};
use crate::canvas::{
    DragPreview,
    drag_preview::{Rgba, drag_cursor, imaging},
};
use windows::Win32::{
    Foundation::POINT,
    Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC,
        GetDIBits, GetMonitorInfoW, GetObjectW, HGDIOBJ, MONITOR_DEFAULTTONEAREST, MONITORINFO,
        MonitorFromPoint, ReleaseDC,
    },
    UI::{
        HiDpi::{GetDpiForMonitor, GetSystemMetricsForDpi, MDT_EFFECTIVE_DPI},
        WindowsAndMessaging::{
            GetCursor, GetCursorPos, GetIconInfo, HICON, ICONINFO, IDC_ARROW, IDC_NO, LoadCursorW,
            SM_CXCURSOR, SetCursor,
        },
    },
};

/// Qt's copy-drag artwork at the sizes its Windows platform plugin ships, arrow tip at (0, 0).
const COPY_ARTWORK: [(u32, &[u8]); 3] = [
    (32, include_bytes!("../../../../assets/cursors/dragcopycursor_32.png")),
    (48, include_bytes!("../../../../assets/cursors/dragcopycursor_48.png")),
    (64, include_bytes!("../../../../assets/cursors/dragcopycursor_64.png")),
];

/// The two cursors one drag shows; they outlive the drag source that selects them.
pub(super) struct DragCursors {
    copy: OwnedCursor,
    refused: OwnedCursor,
}

impl DragCursors {
    /// Compose both cursors for `preview` on the monitor under the pointer. Call on the drag's
    /// UI thread before the modal drag starts.
    ///
    /// # Errors
    /// Preserves monitor, imaging and cursor-creation failures.
    pub(super) fn new(preview: &DragPreview) -> Result<Self, DesktopError> {
        let (available, cursor_size) = pointer_monitor()?;
        let image = preview.fitted(available)?;
        let copy = drag_cursor(&image, &copy_artwork(cursor_size)?)?;
        let refused = drag_cursor(&image, &unavailable_artwork()?)?;
        Ok(Self { copy: cursor(&copy)?, refused: cursor(&refused)? })
    }

    /// Select the cursor for the target's offered `effect`.
    pub(super) fn select(&self, effect: DROPEFFECT) {
        let cursor = if effect.0 & DROPEFFECT_COPY.0 != 0 { &self.copy } else { &self.refused };
        // SAFETY: Selection on the drag's UI thread; this owner keeps the cursor alive.
        unsafe { SetCursor(Some(cursor.handle())) };
    }

    /// Restore the shared arrow when one of these cursors is still selected, so they can be
    /// destroyed after the drag ends.
    pub(super) fn deselect(&self) {
        // SAFETY: Read and selection on the drag's UI thread; the arrow is a shared cursor.
        unsafe {
            let current = GetCursor();
            if current == self.copy.handle() || current == self.refused.handle() {
                SetCursor(LoadCursorW(None, IDC_ARROW).ok());
            }
        }
    }
}

/// The physical available area of the monitor under the pointer and its native cursor size.
fn pointer_monitor() -> Result<([u32; 2], u32), DesktopError> {
    let failed = |cause| DesktopError::new("drag_preview_monitor", cause);
    let mut point = POINT::default();
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    let mut dpi = [0_u32; 2];
    // SAFETY: Each call copies plain values into live local storage.
    let size = unsafe {
        // A desktop that does not receive input has no pointer position; the primary monitor,
        // which holds the origin, then sizes the preview.
        if GetCursorPos(&raw mut point).is_err() {
            point = POINT::default();
        }
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        if !GetMonitorInfoW(monitor, &raw mut info).as_bool() {
            return Err(DesktopError::message("drag_preview_monitor", "Monitor area unavailable"));
        }
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &raw mut dpi[0], &raw mut dpi[1])
            .map_err(failed)?;
        GetSystemMetricsForDpi(SM_CXCURSOR, dpi[0])
    };
    let work = info.rcWork;
    let available = [work.right - work.left, work.bottom - work.top]
        .map(|edge| u32::try_from(edge).unwrap_or(0).max(1));
    Ok((available, u32::try_from(size).unwrap_or(32).max(1)))
}

/// Qt's copy artwork nearest the native cursor size, scaled to that size when none matches.
fn copy_artwork(cursor_size: u32) -> Result<Rgba, DesktopError> {
    let (edge, bytes) = COPY_ARTWORK
        .into_iter()
        .min_by_key(|(edge, _)| edge.abs_diff(cursor_size))
        .ok_or_else(|| DesktopError::message("drag_cursor_artwork", "Copy artwork missing"))?;
    let decoded = imaging::decode(bytes, [edge; 2], |_| Ok(()))?;
    let art = Rgba { size: [decoded.width, decoded.height], pixels: decoded.rgba };
    if edge == cursor_size {
        return Ok(art);
    }
    let size = [cursor_size; 2];
    Ok(Rgba { size, pixels: imaging::rescale(&art.pixels, art.size, size)? })
}

/// The colour pixels of the system "unavailable" cursor, as Qt reads them for a refused drop.
fn unavailable_artwork() -> Result<Rgba, DesktopError> {
    let failed = |cause| DesktopError::new("drag_cursor_unavailable", cause);
    // SAFETY: The shared system cursor is never destroyed. GetIconInfo hands over copies of its
    // bitmaps, which this function deletes; GetDIBits writes at most the requested rows into a
    // buffer sized for them.
    unsafe {
        let shared = LoadCursorW(None, IDC_NO).map_err(failed)?;
        let mut info = ICONINFO::default();
        GetIconInfo(HICON(shared.0), &raw mut info).map_err(failed)?;
        let delete = |bitmap: windows::Win32::Graphics::Gdi::HBITMAP| {
            if !bitmap.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
            }
        };
        let result = (|| {
            if info.hbmColor.is_invalid() {
                return Err(DesktopError::message("drag_cursor_unavailable", "No colour plane"));
            }
            let mut plane = BITMAP::default();
            if GetObjectW(
                HGDIOBJ(info.hbmColor.0),
                size_of::<BITMAP>() as i32,
                Some((&raw mut plane).cast()),
            ) == 0
            {
                return Err(DesktopError::message("drag_cursor_unavailable", "Plane unreadable"));
            }
            let size = [plane.bmWidth, plane.bmHeight].map(|edge| u32::try_from(edge).unwrap_or(0));
            if size.contains(&0) {
                return Err(DesktopError::message("drag_cursor_unavailable", "Plane is empty"));
            }
            let mut header = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: plane.bmWidth,
                    biHeight: -plane.bmHeight,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut pixels = vec![0_u8; size[0] as usize * size[1] as usize * 4];
            let screen = GetDC(None);
            let rows = GetDIBits(
                screen,
                info.hbmColor,
                0,
                size[1],
                Some(pixels.as_mut_ptr().cast()),
                &raw mut header,
                DIB_RGB_COLORS,
            );
            ReleaseDC(None, screen);
            if rows != plane.bmHeight {
                return Err(DesktopError::message("drag_cursor_unavailable", "Plane unread"));
            }
            pixels.chunks_exact_mut(4).for_each(|pixel| pixel.swap(0, 2));
            Ok(Rgba { size, pixels })
        })();
        delete(info.hbmColor);
        delete(info.hbmMask);
        result
    }
}

/// A private cursor showing `image` with its hotspot at the top-left corner.
fn cursor(image: &Rgba) -> Result<OwnedCursor, DesktopError> {
    rgba_cursor(image.size, [0, 0], &image.pixels)
        .map_err(|cause| DesktopError::boxed("drag_cursor_create", cause))
}

#[cfg(test)]
mod tests;
