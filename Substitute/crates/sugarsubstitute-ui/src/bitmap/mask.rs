// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Data-only coverage masks retain raster bounds without exposing path-based image APIs.
use tiny_skia::IntSize;

/// Owned one-byte-per-pixel coverage. Callers retain admission for its original allocation.
/// ```compile_fail
/// use sugarsubstitute_ui::bitmap::CoverageMask;
/// fn bypass(mask: CoverageMask) { let raw_mask = mask.0; }
/// ```
pub struct CoverageMask(tiny_skia::Mask);
impl CoverageMask {
    /// Allocate zeroed coverage after admission; invalid dimensions return `None`.
    pub fn new(width: u32, height: u32) -> Option<Self> {
        tiny_skia::Mask::new(width, height).map(Self)
    }
    /// Adopt exact one-byte coverage without copying or growing its admitted allocation.
    pub fn from_vec(bytes: Vec<u8>, size: IntSize) -> Option<Self> {
        tiny_skia::Mask::from_vec(bytes, size).map(Self)
    }
    /// Physical mask width in pixels.
    pub fn width(&self) -> u32 {
        self.0.width()
    }
    /// Physical mask height in pixels.
    pub fn height(&self) -> u32 {
        self.0.height()
    }
    /// Borrow row-major coverage bytes.
    pub fn data(&self) -> &[u8] {
        self.0.data()
    }
    /// Borrow the existing admitted coverage allocation for bounded painting.
    pub fn data_mut(&mut self) -> &mut [u8] {
        self.0.data_mut()
    }
    /// Lend the backing mask only within the authoritative shared renderer.
    pub(in crate::bitmap) fn raw(&self) -> &tiny_skia::Mask {
        &self.0
    }
}
