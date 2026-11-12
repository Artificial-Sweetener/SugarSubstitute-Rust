// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Original Qt one-device-pixel geometry and licensed close-path coverage at native DPI.
use super::pixels::Pixels;
use crate::DesktopError;

const CLOSE: &[u8] = include_bytes!("../assets/caption/close.coverage");

/// Preserve Qt's antialiased close-button rectangle at the final fractional device edge.
pub(super) fn fill_close_background(
    pixels: &mut Pixels<'_>,
    dpi: u32,
    rgb: [u8; 3],
) -> Result<(), DesktopError> {
    if !(48..=768).contains(&dpi) {
        return Err(DesktopError::message("caption_close_dpi", "Unsupported caption device scale"));
    }
    let index = 16 + (dpi - 48) as usize * 4;
    let invalid =
        || DesktopError::message("caption_close_record", "Invalid compiled caption record");
    let offset = u32::from_le_bytes(
        CLOSE
            .get(index..index + 4)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|cause| DesktopError::new("caption_close_record", cause))?,
    ) as usize;
    let [right, bottom, corner] =
        <[u8; 3]>::try_from(CLOSE.get(offset + 4..offset + 7).ok_or_else(invalid)?)
            .map_err(|cause| DesktopError::new("caption_close_edges", cause))?;
    let width = (46 * dpi).div_ceil(96) as usize;
    let height = (32 * dpi).div_ceil(96) as usize;
    for y in 0..pixels.height {
        for x in 0..pixels.width {
            let coverage = if x >= width || y >= height {
                0
            } else if x == width - 1 && y == height - 1 {
                corner
            } else if x == width - 1 {
                right
            } else if y == height - 1 {
                bottom
            } else {
                255
            };
            pixels.over_color(x, y, rgb, coverage);
        }
    }
    Ok(())
}

/// Paint the authoritative vector geometry without font substitution or scale interpolation.
pub(super) fn draw(
    pixels: &mut Pixels<'_>,
    id: u32,
    maximized: bool,
    dpi: u32,
    color: u8,
) -> Result<(), DesktopError> {
    let scale = |value: usize| value * dpi as usize / 96;
    let mut line = |x0: usize, y0: usize, x1: usize, y1: usize| {
        for y in y0..=y1 {
            for x in x0..=x1 {
                pixels.over(x, y, color, 255);
            }
        }
    };
    match id {
        1 => line(scale(18), scale(16), scale(28), scale(16)),
        2 => {
            let x = scale(18);
            let y = scale(if maximized { 13 } else { 11 });
            let side = scale(if maximized { 8 } else { 10 });
            line(x, y, x + side, y);
            line(x, y + side, x + side, y + side);
            line(x, y + 1, x, y + side.saturating_sub(1));
            line(x + side, y + 1, x + side, y + side.saturating_sub(1));
            if maximized {
                let delta = scale(2);
                line(x + delta, y - delta, x + delta + side, y - delta);
                line(x + delta, y - delta + 1, x + delta, y.saturating_sub(1));
                line(x + delta + side, y - delta + 1, x + delta + side, y - delta + side);
                line(x + side + 1, y - delta + side, x + delta + side - 1, y - delta + side);
            }
        }
        3 => return close(pixels, dpi, color, CLOSE),
        _ => return Err(DesktopError::message("caption_vector_id", "Unknown caption glyph")),
    }
    Ok(())
}

/// The update button's arrow into a tray, in the caption buttons' 10-pixel glyph box, drawn
/// whole in `tint` so ready and warning read at a glance.
pub(super) fn update(pixels: &mut Pixels<'_>, dpi: u32, tint: [u8; 3]) {
    let scale = |value: usize| value * dpi as usize / 96;
    let stroke = scale(1).max(1);
    let mut dot = |x: usize, y: usize| {
        for dy in 0..stroke {
            for dx in 0..stroke {
                pixels.over_color(x + dx, y + dy, tint, 255);
            }
        }
    };
    let (left, top, side) = (scale(18), scale(11), scale(10));
    let middle = left + side / 2;
    for y in top..=top + side * 7 / 10 {
        dot(middle, y);
    }
    // The arrowhead's two diagonals meet at the stem's foot.
    let head = side * 4 / 10;
    let foot = top + side * 7 / 10;
    for step in 0..=head {
        dot(middle - step, foot - step);
        dot(middle + step, foot - step);
    }
    for x in left..=left + side {
        dot(x, top + side);
    }
}

/// Tint of the update glyph: Fluent success green when ready, caution amber after a failure.
pub(super) const fn update_tint(warning: bool, dark: bool) -> [u8; 3] {
    match (warning, dark) {
        (false, false) => [0x0f, 0x7b, 0x0f],
        (false, true) => [0x6c, 0xcb, 0x5f],
        (true, false) => [0x9d, 0x5d, 0x00],
        (true, true) => [0xfc, 0xe1, 0x00],
    }
}

/// Validate every resource boundary before publishing either original SVG path.
fn close(pixels: &mut Pixels<'_>, dpi: u32, color: u8, bytes: &[u8]) -> Result<(), DesktopError> {
    let invalid = || {
        DesktopError::message("caption_close_coverage", "Invalid or unsupported caption coverage")
    };
    let read16 = |offset| {
        bytes
            .get(offset..offset + 2)
            .and_then(|v| <[u8; 2]>::try_from(v).ok())
            .map(u16::from_le_bytes)
    };
    let read32 = |offset| {
        bytes
            .get(offset..offset + 4)
            .and_then(|v| <[u8; 4]>::try_from(v).ok())
            .map(u32::from_le_bytes)
    };
    if bytes.len() > 1024 * 1024 || bytes.get(..8) != Some(b"SSCAPA1\0") {
        return Err(invalid());
    }
    let first = u32::from(read16(8).ok_or_else(invalid)?);
    let last = u32::from(read16(10).ok_or_else(invalid)?);
    let count = read32(12).ok_or_else(invalid)?;
    if first != 48 || last != 768 || count != last - first + 1 || !(first..=last).contains(&dpi) {
        return Err(invalid());
    }
    if read32(16 + count as usize * 4).map(|offset| offset as usize) != Some(bytes.len()) {
        return Err(invalid());
    }
    let index = 16 + (dpi - first) as usize * 4;
    let start = read32(index).ok_or_else(invalid)? as usize;
    let end = read32(index + 4).ok_or_else(invalid)? as usize;
    if start < 16 + (count as usize + 1) * 4 || end > bytes.len() || end <= start {
        return Err(invalid());
    }
    let width = usize::from(read16(start).ok_or_else(invalid)?);
    let height = usize::from(read16(start + 2).ok_or_else(invalid)?);
    // Child edges round independently at fractional DPI; the original logical vector stays fixed.
    if width != ((46 * dpi + 48) / 96) as usize
        || height != ((32 * dpi + 48) / 96) as usize
        || pixels.width.abs_diff(width) > 1
        || pixels.height.abs_diff(height) > 1
    {
        return Err(invalid());
    }
    let mut cursor = start + 7;
    for _ in 0..2 {
        let runs = read32(cursor).ok_or_else(invalid)? as usize;
        cursor += 4;
        if runs > width * height {
            return Err(invalid());
        }
        for _ in 0..runs {
            let y = usize::from(read16(cursor).ok_or_else(invalid)?);
            let x = usize::from(read16(cursor + 2).ok_or_else(invalid)?);
            let length = usize::from(read16(cursor + 4).ok_or_else(invalid)?);
            cursor += 6;
            let coverage = bytes
                .get(cursor..cursor + length)
                .filter(|_| cursor + length <= end)
                .ok_or_else(invalid)?;
            if y >= height || length == 0 || x + length > width || coverage.contains(&0) {
                return Err(invalid());
            }
            for (offset, alpha) in coverage.iter().copied().enumerate() {
                pixels.over(x + offset, y, color, alpha);
            }
            cursor += length;
        }
    }
    if cursor != end {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{buffered, pixels};
    use super::*;
    use windows::Win32::{
        Foundation::RECT,
        Graphics::Gdi::{CreateCompatibleDC, DeleteDC},
    };

    /// Resource corruption and unsupported device scales cannot publish a caption frame.
    #[test]
    fn native_caption_coverage_rejects_corrupt_records_and_unsupported_dpi()
    -> Result<(), DesktopError> {
        // SAFETY: This owned memory DC creates no window or desktop surface.
        let dc = unsafe { CreateCompatibleDC(None) };
        assert!(!dc.is_invalid());
        let rect = RECT { left: 0, top: 0, right: 46, bottom: 32 };
        let result = buffered(dc, rect, |_, buffer| {
            pixels::with_pixels(buffer, rect, |pixels| {
                for dpi in [0, 47, 769, u32::MAX] {
                    assert!(close(pixels, dpi, 255, CLOSE).is_err());
                }
                for length in [0, 8, 16, 100, CLOSE.len() - 1] {
                    assert!(close(pixels, 96, 255, &CLOSE[..length]).is_err());
                }
                let mut corrupt = CLOSE.to_vec();
                corrupt[0] ^= 1;
                assert!(close(pixels, 96, 255, &corrupt).is_err());
                let index = 16 + (96 - 48) * 4;
                let offset = u32::from_le_bytes(
                    CLOSE[index..index + 4]
                        .try_into()
                        .map_err(|cause| DesktopError::new("caption_test_record", cause))?,
                ) as usize;
                corrupt.copy_from_slice(CLOSE);
                corrupt[offset] = 0;
                assert!(close(pixels, 96, 255, &corrupt).is_err());
                corrupt.copy_from_slice(CLOSE);
                corrupt[offset + 7..offset + 11].fill(255);
                assert!(close(pixels, 96, 255, &corrupt).is_err());
                Ok(())
            })
        });
        // SAFETY: The native painting transaction has released the DC on every return path.
        assert!(unsafe { DeleteDC(dc) }.as_bool());
        result
    }
}
