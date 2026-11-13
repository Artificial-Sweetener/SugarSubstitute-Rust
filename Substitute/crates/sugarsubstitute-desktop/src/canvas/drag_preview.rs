// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare the image an OS drag-out of the shown output carries under the pointer.
//!
//! CuteCanvas's Qt drag scaled the dragged image to fit 15% of the available area of the
//! pointer's monitor with Qt's smooth filter, keeping its aspect ratio and enlarging small
//! images, and drew it 10 pixels right of and below the drag cursor. The canvas owner decodes the
//! output on its drag-out worker, bounded by [`DRAG_PREVIEW_CEILING`], because decoding a large
//! output must not stall the UI thread; the drag source then fits that preview to its monitor.

use crate::DesktopError;
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};

#[cfg(windows)]
#[allow(unsafe_code)]
pub(crate) mod imaging;

/// Largest preview the worker prepares: 15% of an 8K monitor on either axis, so the drag source
/// fits it to any monitor without decoding the output again.
pub const DRAG_PREVIEW_CEILING: [u32; 2] = [1152, 1152];

/// Share of the monitor's available area a drag preview fits, as CuteCanvas sized it.
#[cfg(any(target_os = "windows", test))]
pub(crate) const MONITOR_SHARE: f64 = 0.15;

/// One output's drag preview: straight-alpha RGBA rows, top row first, and the output's own size
/// so the drag source sizes the final image exactly as Qt sized it from the original.
pub struct DragPreview {
    source: [u32; 2],
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    _claim: MemoryClaim,
}

impl DragPreview {
    /// Decode encoded output bytes and shrink them, never enlarge, to fit `bound` physical
    /// pixels. Call on an admitted storage worker: decoding runs synchronously. The decoded
    /// output and the kept preview are charged to `memory` while they exist.
    ///
    /// # Errors
    /// Reports unsupported or corrupt images, refused admission and native imaging failures.
    pub fn decode(
        bytes: &[u8],
        bound: [u32; 2],
        memory: &MemoryLease,
    ) -> Result<Self, DesktopError> {
        #[cfg(windows)]
        {
            let decoded = imaging::decode(bytes, bound, |pixels| {
                memory
                    .claim(pixels.saturating_mul(4))
                    .map_err(|cause| DesktopError::new("drag_preview_admission", cause))
            })?;
            let claim = memory
                .claim(decoded.rgba.len() as u64)
                .map_err(|cause| DesktopError::new("drag_preview_admission", cause))?;
            Ok(Self {
                source: decoded.source,
                width: decoded.width,
                height: decoded.height,
                rgba: decoded.rgba,
                _claim: claim,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = (bytes, bound, memory);
            Err(DesktopError::message("drag_preview_decode", "Drag previews require Windows"))
        }
    }

    /// The output's own pixel size, before any scaling.
    pub const fn source(&self) -> [u32; 2] {
        self.source
    }

    /// Preview width in physical pixels.
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Preview height in physical pixels.
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Straight-alpha RGBA bytes, top row first.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// The drag image for a monitor whose available area is `available` physical pixels: the
    /// output fitted to CuteCanvas's 15% share of it, sized from the output's own dimensions.
    /// Call on the drag's UI thread; only this small preview is scaled.
    ///
    /// # Errors
    /// Preserves native imaging failures.
    #[cfg(windows)]
    pub(crate) fn fitted(&self, available: [u32; 2]) -> Result<Rgba, DesktopError> {
        let size = keep_aspect(self.source, monitor_bound(available)).map(|edge| edge.max(1));
        let pixels = if size == [self.width, self.height] {
            self.rgba.clone()
        } else {
            imaging::rescale(&self.rgba, [self.width, self.height], size)?
        };
        Ok(Rgba { size, pixels })
    }
}

/// Scale `size` to fit `bound` keeping its aspect ratio, enlarging or shrinking, with the
/// integer rounding of Qt's `QSize::scaled(bound, Qt::KeepAspectRatio)`.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn keep_aspect(
    [width, height]: [u32; 2],
    [bound_width, bound_height]: [u32; 2],
) -> [u32; 2] {
    if width == 0 || height == 0 {
        return [bound_width, bound_height];
    }
    let fitted_width = u64::from(bound_height) * u64::from(width) / u64::from(height);
    if fitted_width <= u64::from(bound_width) {
        [fitted_width as u32, bound_height]
    } else {
        [bound_width, (u64::from(bound_width) * u64::from(height) / u64::from(width)) as u32]
    }
}

/// The preview size the drag shows on a monitor whose available area is `available` physical
/// pixels: CuteCanvas's 15% share on each axis, rounded half to even as its Python sizing
/// rounded, and at least one pixel.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn monitor_bound(available: [u32; 2]) -> [u32; 2] {
    available.map(|edge| ((f64::from(edge) * MONITOR_SHARE).round_ties_even() as u32).max(1))
}

/// The size the worker keeps for an output of `source` pixels: the output itself when it fits
/// `bound`, otherwise its aspect-preserving fit.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn worker_size(source: [u32; 2], bound: [u32; 2]) -> [u32; 2] {
    if source[0] <= bound[0] && source[1] <= bound[1] {
        source
    } else {
        keep_aspect(source, bound).map(|edge| edge.max(1))
    }
}

/// Straight-alpha RGBA pixels of one drag image or cursor artwork, top row first.
#[derive(Clone, Debug, PartialEq)]
#[cfg(any(target_os = "windows", test))]
pub(crate) struct Rgba {
    pub(crate) size: [u32; 2],
    pub(crate) pixels: Vec<u8>,
}

/// How far right and down of the pointer the preview starts: Qt's default drag hot spot is
/// (-10, -10), which places the drag pixmap 10 pixels from the cursor's own hotspot.
#[cfg(any(target_os = "windows", test))]
pub(crate) const PREVIEW_OFFSET: u32 = 10;

/// Compose the cursor a drag shows, as Qt's Windows drag source composes a drag pixmap with its
/// default hot spot: a transparent canvas one pixel larger than the preview at
/// [`PREVIEW_OFFSET`] or the `cursor` artwork, whichever reaches further, the preview copied in
/// at that offset and the artwork drawn over it source-over at the origin, which is the hotspot.
///
/// # Errors
/// Rejects pixel buffers that do not match their sizes.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn drag_cursor(preview: &Rgba, cursor: &Rgba) -> Result<Rgba, DesktopError> {
    for image in [preview, cursor] {
        if image.pixels.len() as u64 != u64::from(image.size[0]) * u64::from(image.size[1]) * 4 {
            return Err(DesktopError::message("drag_cursor_compose", "Image pixels mismatch"));
        }
    }
    let size = [0, 1].map(|axis| (preview.size[axis] + PREVIEW_OFFSET).max(cursor.size[axis]) + 1);
    let width = size[0] as usize;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(width * size[1] as usize * 4)
        .map_err(|cause| DesktopError::new("drag_cursor_compose", cause))?;
    pixels.resize(width * size[1] as usize * 4, 0);
    for (layer, image) in [preview, cursor].into_iter().enumerate() {
        let offset = if layer == 0 { PREVIEW_OFFSET as usize } else { 0 };
        let row = image.size[0] as usize * 4;
        for (y, source) in image.pixels.chunks_exact(row).enumerate() {
            let start = ((y + offset) * width + offset) * 4;
            let target = &mut pixels[start..start + row];
            if layer == 0 {
                target.copy_from_slice(source);
            } else {
                target.chunks_exact_mut(4).zip(source.chunks_exact(4)).for_each(over);
            }
        }
    }
    Ok(Rgba { size, pixels })
}

/// Draw one straight-alpha source pixel over a straight-alpha target pixel.
#[cfg(any(target_os = "windows", test))]
fn over((target, source): (&mut [u8], &[u8])) {
    let source_alpha = u32::from(source[3]);
    let target_alpha = u32::from(target[3]) * (255 - source_alpha) / 255;
    let alpha = source_alpha + target_alpha;
    if alpha == 0 {
        target.fill(0);
        return;
    }
    for channel in 0..3 {
        let blended =
            u32::from(source[channel]) * source_alpha + u32::from(target[channel]) * target_alpha;
        target[channel] = ((blended + alpha / 2) / alpha) as u8;
    }
    target[3] = alpha as u8;
}

#[cfg(test)]
pub(crate) mod tests;
