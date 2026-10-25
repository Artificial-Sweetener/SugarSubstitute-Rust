// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Checked, origin-relative image extents and half-open pixel regions.
use crate::ImageError;

/// Nonempty finite raster extent in image pixels, independent of screen DPI.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Extent {
    width: u32,
    height: u32,
}
impl Extent {
    /// Validate dimensions without allocating proportional image storage.
    /// # Errors
    /// Rejects zero dimensions or coordinates outside the signed 32-bit raster range.
    pub fn new(width: u32, height: u32) -> Result<Self, ImageError> {
        if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err(ImageError::Invalid("Invalid image extent"));
        }
        Ok(Self { width, height })
    }
    /// Width in image pixels.
    pub const fn width(self) -> u32 {
        self.width
    }
    /// Height in image pixels.
    pub const fn height(self) -> u32 {
        self.height
    }
    /// Complete image region.
    pub const fn region(self) -> Region {
        Region { x: 0, y: 0, width: self.width, height: self.height }
    }
    /// Number of image pixels, without machine-word truncation.
    pub const fn pixels(self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

/// Nonempty half-open image region; its right and bottom edges cannot overflow.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Region {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}
impl Region {
    /// Construct checked image coordinates; callers separately clip to the owning extent.
    /// # Errors
    /// Rejects empty or overflowing regions.
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Result<Self, ImageError> {
        if width == 0
            || height == 0
            || x.checked_add(width).is_none()
            || y.checked_add(height).is_none()
        {
            return Err(ImageError::Invalid("Invalid image region"));
        }
        Ok(Self { x, y, width, height })
    }
    /// Inclusive left edge in image pixels.
    pub const fn x(self) -> u32 {
        self.x
    }
    /// Inclusive top edge in image pixels.
    pub const fn y(self) -> u32 {
        self.y
    }
    /// Region width in pixels.
    pub const fn width(self) -> u32 {
        self.width
    }
    /// Region height in pixels.
    pub const fn height(self) -> u32 {
        self.height
    }
    /// Exclusive right edge.
    pub const fn right(self) -> u32 {
        self.x + self.width
    }
    /// Exclusive bottom edge.
    pub const fn bottom(self) -> u32 {
        self.y + self.height
    }
    /// Number of region pixels in a widened integer.
    pub const fn pixels(self) -> u64 {
        self.width as u64 * self.height as u64
    }
    /// Intersect image regions; disjoint or merely touching regions have no pixels.
    pub fn intersection(self, other: Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        (x < right && y < bottom).then(|| Self { x, y, width: right - x, height: bottom - y })
    }
    /// Whether a pixel belongs to this half-open region.
    pub const fn contains(self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

impl Extent {
    /// Clip an aligned 128-pixel tile to this image's right and bottom edges.
    /// # Errors
    /// Rejects overflowing tile indices or a tile outside the image.
    pub fn tile_region(self, x: u32, y: u32) -> Result<Region, ImageError> {
        let left = x
            .checked_mul(crate::TILE_EDGE)
            .ok_or(ImageError::Invalid("Tile coordinate overflow"))?;
        let top = y
            .checked_mul(crate::TILE_EDGE)
            .ok_or(ImageError::Invalid("Tile coordinate overflow"))?;
        if left >= self.width() || top >= self.height() {
            return Err(ImageError::Invalid("Tile outside image"));
        }
        Region::new(
            left,
            top,
            crate::TILE_EDGE.min(self.width() - left),
            crate::TILE_EDGE.min(self.height() - top),
        )
    }
}
