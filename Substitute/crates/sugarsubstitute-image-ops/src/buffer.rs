// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Couple canonical pixel allocations to retained execution claims.
use crate::{ImageError, Pixel, Region};
use sugarsubstitute_contracts::execution::MemoryClaim;

/// Canonical evaluation tile edge in image pixels, independent of viewport size and DPI.
pub const TILE_EDGE: u32 = 128;

/// Immutable evaluated pixels retaining their original execution memory claim.
/// Cloning an `Arc<Tile>` shares pixels and the same charge; it never duplicates storage.
#[derive(Debug)]
pub struct Tile {
    pub(crate) buffer: Buffer,
}
impl Tile {
    /// Image-space half-open region covered by these pixels.
    pub const fn region(&self) -> Region {
        self.buffer.region
    }
    /// Row-major linear premultiplied pixels, with no row padding.
    pub fn pixels(&self) -> &[Pixel] {
        &self.buffer.pixels
    }
    /// Inspect one image-coordinate pixel, returning `None` outside this tile.
    pub fn pixel(&self, x: u32, y: u32) -> Option<Pixel> {
        self.buffer.pixel(x, y)
    }
    /// Admitted payload bytes held until the final tile owner releases the allocation.
    pub fn payload_bytes(&self) -> u64 {
        self.buffer._memory.bytes()
    }
}

/// Allocation fields precede their claim so actual storage drops before capacity becomes reusable.
#[derive(Debug)]
pub(crate) struct Buffer {
    pub(crate) pixels: Vec<Pixel>,
    pub(crate) region: Region,
    _memory: MemoryClaim,
}
impl Buffer {
    pub(crate) fn new(region: Region, memory: MemoryClaim) -> Result<Self, ImageError> {
        let length = usize::try_from(region.pixels())
            .map_err(|_| ImageError::Invalid("Image buffer exceeds address space"))?;
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(length)?;
        pixels.resize(length, Pixel::TRANSPARENT);
        Ok(Self { pixels, region, _memory: memory })
    }
    pub(crate) fn pixel(&self, x: u32, y: u32) -> Option<Pixel> {
        if !self.region.contains(x, y) {
            return None;
        }
        let index = (y - self.region.y()) as usize * self.region.width() as usize
            + (x - self.region.x()) as usize;
        Some(self.pixels[index])
    }
}
