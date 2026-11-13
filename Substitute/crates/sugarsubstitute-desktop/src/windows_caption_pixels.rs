// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bounded premultiplied pixel access during one retained UxTheme paint transaction.
use crate::DesktopError;
use windows::Win32::{
    Foundation::RECT, Graphics::Gdi::RGBQUAD, UI::Controls::GetBufferedPaintBits,
};

/// A borrowed native buffer cannot escape the synchronous painting closure.
pub(super) struct Pixels<'a> {
    rows: &'a mut [RGBQUAD],
    stride: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

/// Borrow only the pixels belonging to the current top-down buffered-paint rectangle.
pub(super) fn with_pixels<T>(
    buffer: isize,
    rect: RECT,
    draw: impl FnOnce(&mut Pixels<'_>) -> Result<T, DesktopError>,
) -> Result<T, DesktopError> {
    let mut bits = std::ptr::null_mut();
    let mut stride = 0;
    // SAFETY: The caller retains this paint transaction for the complete closure.
    unsafe { GetBufferedPaintBits(buffer, &raw mut bits, &raw mut stride) }
        .map_err(|cause| DesktopError::new("caption_pixel_buffer", cause))?;
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if bits.is_null()
        || rect.left != 0
        || rect.top != 0
        || width <= 0
        || height <= 0
        || stride < width
    {
        return Err(DesktopError::message("caption_pixel_shape", "Invalid native caption buffer"));
    }
    let count = (stride as usize)
        .checked_mul(height as usize)
        .filter(|n| *n <= isize::MAX as usize / size_of::<RGBQUAD>())
        .ok_or_else(|| {
            DesktopError::message("caption_pixel_shape", "Caption buffer exceeds bounds")
        })?;
    // SAFETY: UxTheme owns stride pixels per requested row until EndBufferedPaint; no alias escapes.
    let rows = unsafe { std::slice::from_raw_parts_mut(bits, count) };
    draw(&mut Pixels {
        rows,
        stride: stride as usize,
        width: width as usize,
        height: height as usize,
    })
}

/// Qt's integer division preserves coverage and premultiplication rounding.
fn divide255(value: u32) -> u8 {
    ((value + (value >> 8) + 128) >> 8) as u8
}

impl Pixels<'_> {
    /// Fill the owned rectangle from an unpremultiplied RGBA color.
    pub(super) fn fill(&mut self, rgba: [u8; 4]) {
        let [r, g, b, a] = rgba;
        let pixel = RGBQUAD {
            rgbRed: divide255(u32::from(r) * u32::from(a)),
            rgbGreen: divide255(u32::from(g) * u32::from(a)),
            rgbBlue: divide255(u32::from(b) * u32::from(a)),
            rgbReserved: a,
        };
        for row in self.rows.chunks_exact_mut(self.stride).take(self.height) {
            row[..self.width].fill(pixel);
        }
    }

    /// Compose one original vector-path coverage sample with integer SourceOver.
    pub(super) fn over(&mut self, x: usize, y: usize, color: u8, alpha: u8) {
        self.over_color(x, y, [color; 3], alpha);
    }

    /// Compose a colored original fill without losing its independent path coverage.
    pub(super) fn over_color(&mut self, x: usize, y: usize, color: [u8; 3], alpha: u8) {
        if x >= self.width || y >= self.height {
            return;
        }
        let pixel = &mut self.rows[y * self.stride + x];
        let inverse = u32::from(255 - alpha);
        pixel.rgbRed = divide255(u32::from(color[0]) * u32::from(alpha))
            .saturating_add(divide255(u32::from(pixel.rgbRed) * inverse));
        pixel.rgbGreen = divide255(u32::from(color[1]) * u32::from(alpha))
            .saturating_add(divide255(u32::from(pixel.rgbGreen) * inverse));
        pixel.rgbBlue = divide255(u32::from(color[2]) * u32::from(alpha))
            .saturating_add(divide255(u32::from(pixel.rgbBlue) * inverse));
        pixel.rgbReserved = alpha.saturating_add(divide255(u32::from(pixel.rgbReserved) * inverse));
    }

    /// Count the opaque pixels of exactly `color` and their bounding box `[left, top, right,
    /// bottom]`, inclusive.
    #[cfg(test)]
    pub(super) fn ink(&self, color: [u8; 3]) -> (usize, [usize; 4]) {
        let mut count = 0;
        let mut bounds = [usize::MAX, usize::MAX, 0, 0];
        for (y, row) in self.rows.chunks_exact(self.stride).take(self.height).enumerate() {
            for (x, pixel) in row[..self.width].iter().enumerate() {
                if pixel.rgbReserved == 255
                    && [pixel.rgbRed, pixel.rgbGreen, pixel.rgbBlue] == color
                {
                    count += 1;
                    bounds =
                        [bounds[0].min(x), bounds[1].min(y), bounds[2].max(x), bounds[3].max(y)];
                }
            }
        }
        (count, bounds)
    }

    /// Stable complete-pixel regression digest over BGRA rows, excluding native padding.
    #[cfg(test)]
    pub(super) fn digest(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64;
        for row in self.rows.chunks_exact(self.stride).take(self.height) {
            for pixel in &row[..self.width] {
                for byte in [pixel.rgbBlue, pixel.rgbGreen, pixel.rgbRed, pixel.rgbReserved] {
                    hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
                }
            }
        }
        hash
    }
}
