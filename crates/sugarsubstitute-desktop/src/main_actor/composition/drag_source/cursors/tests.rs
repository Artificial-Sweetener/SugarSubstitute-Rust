// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read the composed drag cursors back from Windows to prove what a drag-out shows.

use super::*;
use crate::canvas::drag_preview::{PREVIEW_OFFSET, keep_aspect, monitor_bound, tests::png};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct Released(Arc<AtomicU64>);
impl MemoryRelease for Released {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::SeqCst);
    }
}

/// Copy one private cursor's colour plane and hotspot back from Windows.
fn read_back(cursor: &OwnedCursor) -> Result<(Rgba, [u32; 2]), Box<dyn std::error::Error>> {
    let mut info = ICONINFO::default();
    // SAFETY: Queries one live private cursor; the returned bitmap copies are deleted below and
    // GetDIBits writes at most the requested rows into a buffer sized for them.
    unsafe {
        GetIconInfo(HICON(cursor.handle().0), &raw mut info)?;
        let mut plane = BITMAP::default();
        GetObjectW(
            HGDIOBJ(info.hbmColor.0),
            size_of::<BITMAP>() as i32,
            Some((&raw mut plane).cast()),
        );
        let size = [plane.bmWidth as u32, plane.bmHeight as u32];
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
        let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
        let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
        assert_eq!(rows, plane.bmHeight);
        pixels.chunks_exact_mut(4).for_each(|pixel| pixel.swap(0, 2));
        Ok((Rgba { size, pixels }, [info.xHotspot, info.yHotspot]))
    }
}

fn at(image: &Rgba, [x, y]: [u32; 2]) -> [u8; 4] {
    let start = ((y * image.size[0] + x) * 4) as usize;
    [0, 1, 2, 3].map(|channel| image.pixels[start + channel])
}

#[test]
fn a_drag_out_cursor_is_qt_copy_artwork_with_the_fitted_output_beside_the_pointer() -> TestResult {
    let lease = MemoryLease::new(1 << 26, Box::new(Released(Arc::new(AtomicU64::new(0)))));
    let colour = [40, 120, 200, 255];
    let output = png([512, 512], &colour.repeat(512 * 512));
    let preview = DragPreview::decode(&output, crate::canvas::DRAG_PREVIEW_CEILING, &lease)?;
    let cursors = DragCursors::new(&preview)?;
    let (available, cursor_size) = pointer_monitor()?;
    let fitted = keep_aspect([512, 512], monitor_bound(available));
    let far = fitted.map(|edge| edge + PREVIEW_OFFSET - 1);
    let art = copy_artwork(cursor_size)?;

    let (copy, hotspot) = read_back(&cursors.copy)?;
    assert_eq!(hotspot, [0, 0], "the pointer sits at the arrow tip");
    assert_eq!(
        copy.size,
        [0, 1].map(|axis| (fitted[axis] + PREVIEW_OFFSET).max(art.size[axis]) + 1)
    );
    assert_eq!(at(&copy, far), colour, "the output fills the offset preview");
    assert_eq!(at(&copy, far.map(|edge| edge + 1))[3], 0, "the extra edge stays transparent");
    // The arrow tip of Qt's copy artwork is the hotspot pixel, drawn over nothing.
    assert_ne!(at(&art, [0, 0])[3], 0);
    assert_eq!(at(&copy, [0, 0]), at(&art, [0, 0]));

    let (refused, hotspot) = read_back(&cursors.refused)?;
    assert_eq!(hotspot, [0, 0]);
    assert_eq!(at(&refused, far), colour);
    Ok(())
}

#[test]
fn a_drag_without_a_preview_keeps_the_standard_cursors() {
    let source: IDropSource = Source(None).into();
    // SAFETY: Calls the drag source's own feedback method on this thread; without a preview it
    // selects no cursor.
    let feedback = unsafe { source.GiveFeedback(DROPEFFECT_COPY) };
    assert_eq!(feedback, DRAGDROP_S_USEDEFAULTCURSORS);
}
