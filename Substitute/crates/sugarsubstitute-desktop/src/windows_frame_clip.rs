// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Protect the independently painted native console during parent animation blits.
use crate::DesktopError;
use windows::Win32::{
    Foundation::RECT,
    Graphics::Gdi::{ERROR, ExcludeClipRect, HDC, RestoreDC, SaveDC},
};

/// Restore the original DC even after a failed blit; Tao retains a private window DC.
/// The exclusion is in physical parent-client pixels and belongs to this submission only.
pub(super) fn submit(
    dc: HDC,
    excluded: Option<RECT>,
    draw: impl FnOnce() -> Result<(), DesktopError>,
) -> Result<(), DesktopError> {
    let Some(rect) = excluded else { return draw() };
    // SAFETY: Caller retains the affine borrowed DC through drawing and restoration.
    let saved = unsafe { SaveDC(dc) };
    if saved == 0 {
        return Err(DesktopError::message("save_frame_clip", "SaveDC failed"));
    }
    // SAFETY: Coordinates are validated physical bounds; only this saved DC is changed.
    let excluded = unsafe { ExcludeClipRect(dc, rect.left, rect.top, rect.right, rect.bottom) };
    let mut failure = if excluded.0 == ERROR {
        Some(DesktopError::message("exclude_console_frame", "ExcludeClipRect failed"))
    } else {
        draw().err()
    };
    // SAFETY: Restore exactly this invocation's save, preserving later hidden/resized layouts.
    if !unsafe { RestoreDC(dc, saved) }.as_bool() {
        DesktopError::retain_first(
            &mut failure,
            DesktopError::message("restore_frame_clip", "RestoreDC failed"),
        );
    }
    failure.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Gdi::{
        CreateBitmap, CreateCompatibleDC, CreateRectRgn, DeleteDC, DeleteObject, PtVisible,
        SelectClipRgn, SelectObject,
    };

    /// Native clipping protects the log while leaving painted buttons and future frames available.
    #[test]
    fn actual_parent_dc_excludes_only_console_and_restores_after_success_and_failure()
    -> Result<(), Box<dyn std::error::Error>> {
        // SAFETY: This test owns the memory DC and region; neither can display a window.
        let dc = unsafe { CreateCompatibleDC(None) };
        assert!(!dc.is_invalid());
        // SAFETY: Allocate a synthetic bitmap; a memory DC initially contains only one pixel.
        let bitmap = unsafe { CreateBitmap(558, 558, 1, 32, None) };
        assert!(!bitmap.is_invalid());
        // SAFETY: Keep the previous borrowed bitmap for restoration before deleting our bitmap.
        let previous = unsafe { SelectObject(dc, bitmap.into()) };
        assert!(!previous.is_invalid());
        // SAFETY: Bounded rectangle creates an owned region, copied into the retained DC.
        let region = unsafe { CreateRectRgn(0, 0, 558, 558) };
        assert!(!region.is_invalid());
        // SAFETY: SelectClipRgn copies the region; deletion cannot invalidate the DC's copy.
        unsafe {
            assert_ne!(SelectClipRgn(dc, Some(region)).0, ERROR);
            assert!(DeleteObject(region.into()).as_bool());
        }
        let console = RECT { left: 30, top: 362, right: 528, bottom: 528 };
        let mut facts = Vec::new();
        for fail in [false, true] {
            let mut clipped = false;
            let mut button_available = false;
            let result = submit(dc, Some(console), || {
                // SAFETY: Passive queries inspect this test's exact retained memory DC.
                unsafe {
                    clipped = !PtVisible(dc, 40, 400).as_bool();
                    button_available = PtVisible(dc, 480, 16).as_bool();
                }
                if fail {
                    Err(DesktopError::message("test_frame", "original failure"))
                } else {
                    Ok(())
                }
            });
            // SAFETY: This private DC remains retained after both transaction outcomes.
            let restored = unsafe { PtVisible(dc, 40, 400) }.as_bool();
            facts.push((result.is_err() == fail, clipped, button_available, restored));
        }
        let mut collapsed_available = false;
        let collapsed = submit(dc, None, || {
            // SAFETY: A collapsed console leaves this same parent-client region paintable.
            collapsed_available = unsafe { PtVisible(dc, 40, 400) }.as_bool();
            Ok(())
        });
        // SAFETY: Restore the borrowed stock bitmap before retiring the exact owned objects.
        unsafe {
            assert_eq!(SelectObject(dc, previous), bitmap.into());
            assert!(DeleteObject(bitmap.into()).as_bool());
            assert!(DeleteDC(dc).as_bool());
        }
        assert_eq!(facts, [(true, true, true, true); 2]);
        collapsed?;
        assert!(collapsed_available);
        Ok(())
    }
}
