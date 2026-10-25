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
    /// Number of aligned image tiles intersected by this exact region, without allocating them.
    pub const fn tile_count(self) -> u64 {
        let columns = (self.right() - 1) / crate::TILE_EDGE - self.x / crate::TILE_EDGE + 1;
        let rows = (self.bottom() - 1) / crate::TILE_EDGE - self.y / crate::TILE_EDGE + 1;
        columns as u64 * rows as u64
    }
    /// Split into row-major clipped intersections with the shared image tile grid.
    /// Every output is nonempty; exact repair edges and global coordinates remain unchanged.
    /// Iteration allocates no proportional storage, including at the u32 coordinate boundary.
    pub fn tiles(self) -> RegionTiles {
        RegionTiles {
            region: self,
            column: self.x / crate::TILE_EDGE,
            row: self.y / crate::TILE_EDGE,
        }
    }
}

/// Owned row-major iterator over globally aligned tile intersections of one exact region.
/// Retain it across cooperative evaluation steps without allocating an inventory of tiles.
#[derive(Clone, Debug)]
pub struct RegionTiles {
    region: Region,
    column: u32,
    row: u32,
}
impl Iterator for RegionTiles {
    type Item = Region;
    fn next(&mut self) -> Option<Self::Item> {
        if self.row > (self.region.bottom() - 1) / crate::TILE_EDGE {
            return None;
        }
        let left = self.region.x.max(self.column * crate::TILE_EDGE);
        let top = self.region.y.max(self.row * crate::TILE_EDGE);
        let right = u64::from(self.region.right())
            .min((u64::from(self.column) + 1) * u64::from(crate::TILE_EDGE))
            as u32;
        let bottom = u64::from(self.region.bottom())
            .min((u64::from(self.row) + 1) * u64::from(crate::TILE_EDGE))
            as u32;
        if self.column == (self.region.right() - 1) / crate::TILE_EDGE {
            self.column = self.region.x / crate::TILE_EDGE;
            self.row += 1;
        } else {
            self.column += 1;
        }
        Some(Region { x: left, y: top, width: right - left, height: bottom - top })
    }
}
impl std::iter::FusedIterator for RegionTiles {}

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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_tile_intersections_preserve_row_order_and_integer_coordinate_limits()
    -> Result<(), ImageError> {
        let region = Region::new(127, 127, 3, 3)?;
        assert_eq!(region.tile_count(), 4);
        assert_eq!(region.tiles().map(Region::pixels).sum::<u64>(), region.pixels());
        let mut tiles = region.tiles();
        for expected in [(127, 127, 1, 1), (128, 127, 2, 1), (127, 128, 1, 2), (128, 128, 2, 2)] {
            assert_eq!(
                tiles.next(),
                Some(Region::new(expected.0, expected.1, expected.2, expected.3)?)
            );
        }
        assert_eq!(tiles.next(), None);
        let edge = Region::new(u32::MAX - 2, u32::MAX - 2, 2, 2)?;
        assert_eq!(edge.tile_count(), 1);
        assert_eq!(edge.tiles().next(), Some(edge));
        Ok(())
    }
}
