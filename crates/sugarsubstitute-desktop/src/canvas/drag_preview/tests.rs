// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Size and compose drag images exactly as CuteCanvas's Qt drag did.

use super::*;

#[test]
fn previews_fit_fifteen_percent_of_the_monitor_with_qt_rounding_enlarging_small_outputs() {
    let bound = monitor_bound([3440, 1410]);
    assert_eq!(bound, [516, 212]);
    // A square output is limited by the short axis.
    assert_eq!(keep_aspect([1024, 1024], bound), [212, 212]);
    // A wide output is limited by the long axis; Qt truncates the derived edge.
    assert_eq!(keep_aspect([3000, 1000], bound), [516, 172]);
    // Small outputs grow to the bound, as Qt's scaled() does.
    assert_eq!(keep_aspect([64, 32], bound), [424, 212]);
    assert_eq!(monitor_bound([1, 1]), [1, 1]);
}

#[test]
fn the_worker_keeps_outputs_that_fit_its_ceiling_and_shrinks_larger_ones() {
    assert_eq!(worker_size([1024, 768], DRAG_PREVIEW_CEILING), [1024, 768]);
    assert_eq!(worker_size([4096, 2048], DRAG_PREVIEW_CEILING), [1152, 576]);
    assert_eq!(worker_size([100_000, 1], DRAG_PREVIEW_CEILING), [1152, 1]);
}

fn solid(size: [u32; 2], pixel: [u8; 4]) -> Rgba {
    Rgba { size, pixels: pixel.repeat((size[0] * size[1]) as usize) }
}

fn at(image: &Rgba, [x, y]: [u32; 2]) -> [u8; 4] {
    let start = ((y * image.size[0] + x) * 4) as usize;
    let mut pixel = [0; 4];
    pixel.copy_from_slice(&image.pixels[start..start + 4]);
    pixel
}

#[test]
fn the_drag_cursor_puts_the_preview_ten_pixels_from_the_pointer_under_its_artwork()
-> Result<(), DesktopError> {
    let preview = solid([6, 4], [200, 100, 50, 255]);
    let mut art = solid([12, 12], [0, 0, 0, 0]);
    art.pixels[..4].copy_from_slice(&[0, 0, 0, 255]);
    let overlap = ((10 * 12 + 11) * 4) as usize;
    art.pixels[overlap..overlap + 4].copy_from_slice(&[255, 255, 255, 128]);
    let cursor = drag_cursor(&preview, &art)?;
    assert_eq!(cursor.size, [17, 15], "one pixel beyond the offset preview");
    assert_eq!(at(&cursor, [0, 0]), [0, 0, 0, 255], "the arrow tip is the hotspot");
    assert_eq!(at(&cursor, [9, 9]), [0, 0, 0, 0], "the offset leaves a transparent gap");
    assert_eq!(at(&cursor, [10, 10]), [200, 100, 50, 255], "the preview starts at the offset");
    assert_eq!(at(&cursor, [11, 10]), [228, 178, 153, 255], "half-transparent artwork blends");
    assert_eq!(at(&cursor, [15, 13]), [200, 100, 50, 255], "the preview's far corner");
    assert_eq!(at(&cursor, [16, 14]), [0, 0, 0, 0], "outside both images stays transparent");
    // Artwork reaching beyond the offset preview extends the canvas.
    let tiny = drag_cursor(&solid([2, 2], [9, 9, 9, 255]), &solid([16, 5], [1, 2, 3, 255]))?;
    assert_eq!(tiny.size, [17, 13]);
    assert!(drag_cursor(&Rgba { size: [2, 2], pixels: vec![0; 3] }, &art).is_err());
    Ok(())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Encode straight RGBA rows as a PNG with stored (uncompressed) deflate blocks.
pub(crate) fn png([width, height]: [u32; 2], rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::new();
    for row in rgba.chunks_exact(width as usize * 4) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut zlib = vec![0x78, 0x01];
    let blocks: Vec<_> = raw.chunks(65_535).collect();
    for (index, block) in blocks.iter().enumerate() {
        zlib.push(u8::from(index + 1 == blocks.len()));
        let length = block.len() as u16;
        zlib.extend_from_slice(&length.to_le_bytes());
        zlib.extend_from_slice(&(!length).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    let (mut a, mut b) = (1_u32, 0_u32);
    for byte in &raw {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut file = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8], data: &[u8]| {
        file.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        file.extend_from_slice(&body);
        file.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &header);
    chunk(b"IDAT", &zlib);
    chunk(b"IEND", &[]);
    file
}

#[cfg(windows)]
mod imaging_tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    use sugarsubstitute_contracts::execution::MemoryRelease;

    struct Released(Arc<AtomicU64>);
    impl MemoryRelease for Released {
        fn release(&self, bytes: u64) {
            self.0.fetch_add(bytes, Ordering::SeqCst);
        }
    }

    #[test]
    fn decoding_keeps_small_outputs_shrinks_large_ones_and_returns_its_admission()
    -> Result<(), Box<dyn std::error::Error>> {
        let lease = MemoryLease::new(1 << 24, Box::new(Released(Arc::new(AtomicU64::new(0)))));
        let red = png([4, 2], &[255, 0, 0, 255].repeat(8));
        let preview = DragPreview::decode(&red, DRAG_PREVIEW_CEILING, &lease)?;
        assert_eq!((preview.source(), preview.width(), preview.height()), ([4, 2], 4, 2));
        assert_eq!(&preview.rgba()[..4], &[255, 0, 0, 255]);
        assert_eq!(lease.claimed_bytes(), 4 * 2 * 4, "only the kept preview stays charged");
        drop(preview);
        assert_eq!(lease.claimed_bytes(), 0);
        // The decoded frame is charged while decoding, so a lease that cannot hold it refuses.
        let narrow = MemoryLease::new(16, Box::new(Released(Arc::new(AtomicU64::new(0)))));
        assert!(DragPreview::decode(&red, DRAG_PREVIEW_CEILING, &narrow).is_err());

        let wide = png([40, 10], &[0, 0, 255, 255].repeat(400));
        let shrunk = DragPreview::decode(&wide, [8, 8], &lease)?;
        assert_eq!((shrunk.source(), shrunk.width(), shrunk.height()), ([40, 10], 8, 2));
        assert!(shrunk.rgba().chunks_exact(4).all(|pixel| pixel == [0, 0, 255, 255]));
        assert!(DragPreview::decode(b"not an image", DRAG_PREVIEW_CEILING, &lease).is_err());
        Ok(())
    }

    #[test]
    fn the_drag_image_is_sized_from_the_original_output_for_its_monitor()
    -> Result<(), Box<dyn std::error::Error>> {
        let lease = MemoryLease::new(1 << 24, Box::new(Released(Arc::new(AtomicU64::new(0)))));
        let wide = png([40, 10], &[0, 255, 0, 255].repeat(400));
        let preview = DragPreview::decode(&wide, [8, 8], &lease)?;
        // 15% of 200x100 is 30x15; the 4:1 original fits as 30x7, enlarged from the kept 8x2.
        let image = preview.fitted([200, 100])?;
        assert_eq!(image.size, [30, 7]);
        assert_eq!(image.pixels.len(), 30 * 7 * 4);
        assert!(image.pixels.chunks_exact(4).all(|pixel| pixel == [0, 255, 0, 255]));
        Ok(())
    }
}
