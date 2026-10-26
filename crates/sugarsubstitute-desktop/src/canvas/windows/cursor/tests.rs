// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate bundled Qt cursor parsing and native Windows resource decoding without a window.

use super::*;
use ::windows::Win32::{
    Foundation::GetLastError,
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDIBits,
        HDC, HGDIOBJ,
    },
    UI::WindowsAndMessaging::{GetIconInfo, HICON, ICONINFO},
};
use std::cell::{Cell, RefCell};

struct DeviceContext(HDC);
impl Drop for DeviceContext {
    fn drop(&mut self) {
        // SAFETY: This exact memory DC was created by the test and is not selected elsewhere.
        if !unsafe { DeleteDC(self.0) }.as_bool() {
            tracing::error!(
                operation = "canvas_cursor_fixture_dc_retirement",
                "Memory DC retirement failed"
            );
        }
    }
}

struct IconCopies(ICONINFO);
impl Drop for IconCopies {
    fn drop(&mut self) {
        // SAFETY: GetIconInfo returned these private bitmap copies to this test.
        for bitmap in [self.0.hbmMask, self.0.hbmColor] {
            // SAFETY: Each non-null bitmap is a distinct private copy from GetIconInfo.
            if !bitmap.is_invalid() && !unsafe { DeleteObject(HGDIOBJ(bitmap.0)) }.as_bool() {
                tracing::error!(
                    operation = "canvas_cursor_fixture_bitmap_retirement",
                    "Cursor bitmap retirement failed"
                );
            }
        }
    }
}

#[test]
fn qt_hand_archives_select_the_nearest_frame_and_authored_hotspot() -> Result<(), DesktopError> {
    for bytes in [OPEN_HAND, CLOSED_HAND] {
        for (requested, extent, hotspot) in [
            ([32, 32], [32, 32], [10, 12]),
            ([47, 47], [48, 48], [15, 16]),
            ([64, 64], [64, 64], [20, 24]),
        ] {
            let frame = select_frame(bytes, requested)?;
            assert_eq!(frame.extent, extent);
            assert_eq!(frame.hotspot, hotspot);
        }
    }
    Ok(())
}

#[test]
fn qt_hand_resources_decode_to_distinct_native_cursors_at_authored_and_scaled_sizes()
-> Result<(), Box<dyn Error>> {
    // SAFETY: Create a memory-only GDI context; this creates no window or input-desktop state.
    let dc = DeviceContext(unsafe { CreateCompatibleDC(None) });
    if dc.0.is_invalid() {
        // SAFETY: Read the failure recorded by the immediately preceding native call.
        let code = unsafe { GetLastError() };
        return Err(DesktopError::new(
            "canvas_cursor_test_dc",
            std::io::Error::from_raw_os_error(code.0 as i32),
        )
        .into());
    }
    for size in [32, 48, 64] {
        let open = load_cursor(OPEN_HAND, [size, size])?;
        let closed = load_cursor(CLOSED_HAND, [size, size])?;
        assert_ne!(open.handle(), closed.handle());
        assert_native_cursor(&open, OPEN_HAND, size, &dc)?;
        assert_native_cursor(&closed, CLOSED_HAND, size, &dc)?;
    }
    let open = load_cursor(OPEN_HAND, [40, 40])?;
    let closed = load_cursor(CLOSED_HAND, [40, 40])?;
    assert_ne!(open.handle(), closed.handle());
    for cursor in [&open, &closed] {
        let info = icon_info(cursor)?;
        assert!(!info.0.fIcon.as_bool());
        assert_scaled_hotspot(info.0.xHotspot, 15, 48, 40);
        assert_scaled_hotspot(info.0.yHotspot, 16, 48, 40);
        let pixels = bitmap_pixels(&info.0, 40, &dc)?;
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
    }
    Ok(())
}

#[test]
fn repeated_selection_skips_native_work_but_reasserts_external_and_tool_changes() {
    let actual = Cell::new(0_u8);
    let selected = RefCell::new(Vec::new());
    let publish = |desired| {
        select_when_changed(
            desired,
            || actual.get(),
            |handle| {
                actual.set(handle);
                selected.borrow_mut().push(handle);
            },
        );
    };

    publish(1);
    publish(1);
    actual.set(2);
    publish(1);
    publish(3);
    publish(3);

    assert_eq!(&*selected.borrow(), &[1, 1, 3]);
    assert_eq!(actual.get(), 3);
}

fn assert_native_cursor(
    cursor: &OwnedCursor,
    archive: &[u8],
    size: i32,
    dc: &DeviceContext,
) -> Result<(), Box<dyn Error>> {
    let info = icon_info(cursor)?;
    assert!(!info.0.fIcon.as_bool());
    let frame = select_frame(archive, [size, size])?;
    assert_eq!(info.0.xHotspot, u32::from(frame.hotspot[0]));
    assert_eq!(info.0.yHotspot, u32::from(frame.hotspot[1]));
    let pixels = bitmap_pixels(&info.0, size, dc)?;
    let row_bytes = size as usize * 4;
    let source = &frame.bytes[40..40 + row_bytes * size as usize];
    for row in 0..size as usize {
        let expected = (size as usize - 1 - row) * row_bytes;
        let actual = row * row_bytes;
        assert_eq!(&pixels[actual..actual + row_bytes], &source[expected..expected + row_bytes]);
    }
    Ok(())
}

fn icon_info(cursor: &OwnedCursor) -> Result<IconCopies, Box<dyn Error>> {
    let mut info = ICONINFO::default();
    // SAFETY: Query one live private cursor; returned bitmap copies are adopted by IconCopies.
    unsafe {
        GetIconInfo(HICON(cursor.handle().0), &mut info)?;
    }
    Ok(IconCopies(info))
}

fn bitmap_pixels(
    info: &ICONINFO,
    size: i32,
    dc: &DeviceContext,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bitmap = BITMAPINFO::default();
    bitmap.bmiHeader.biSize = u32::try_from(size_of_val(&bitmap.bmiHeader))?;
    bitmap.bmiHeader.biWidth = size;
    bitmap.bmiHeader.biHeight = -size;
    bitmap.bmiHeader.biPlanes = 1;
    bitmap.bmiHeader.biBitCount = 32;
    bitmap.bmiHeader.biCompression = BI_RGB.0;
    let mut pixels = vec![0; size as usize * size as usize * 4];
    // SAFETY: Buffer and BITMAPINFO remain valid for the synchronous top-down BGRA copy.
    let lines = unsafe {
        GetDIBits(
            dc.0,
            info.hbmColor,
            0,
            size as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut bitmap,
            DIB_RGB_COLORS,
        )
    };
    if lines != size {
        return Err(DesktopError::message(
            "canvas_cursor_test_pixels",
            "Native cursor bitmap decoding failed",
        )
        .into());
    }
    Ok(pixels)
}

fn assert_scaled_hotspot(actual: u32, source: u32, source_size: u32, target_size: u32) {
    let scaled = source * target_size;
    assert!((actual * source_size).abs_diff(scaled) <= source_size);
}
