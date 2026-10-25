// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound scanline import workspace while retaining contiguous region-addressable tile chunks.
use crate::ImportError;
use sugarsubstitute_image_ops::{Extent, Region, TILE_EDGE};

pub(crate) const HEADER_BYTES: u64 = 128;
const STRIPE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024 * 1024;
const HASH_BYTES: u64 = 32;

/// Straight encoded RGBA samples; transparent source color and original integer precision survive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Packing {
    /// Four 8-bit samples in red, green, blue, alpha order.
    Rgba8,
    /// Four 16-bit big-endian samples in red, green, blue, alpha order.
    Rgba16,
}
impl Packing {
    /// Exact bytes per pixel in the reconstructed source archive.
    pub const fn bytes(self) -> u32 {
        match self {
            Self::Rgba8 => 4,
            Self::Rgba16 => 8,
        }
    }
}

/// Adaptive tile-row packing: chunks are at most 128 pixels wide and 128 rows high.
/// A complete scanline stripe is at most 8 MiB. Extremely wide sources use shorter chunks;
/// importing a 32 GiB image therefore never requires a proportional decoded allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkLayout {
    extent: Extent,
    packing: Packing,
    rows: u32,
    columns: u32,
    stripes: u32,
    length: u64,
}
impl ChunkLayout {
    /// Choose packing using the fixed format's bounded stripe and file-size limits.
    /// # Errors
    /// Rejects an encoded row over 8 MiB or an archive over 32 GiB, including chunk hashes.
    pub fn new(extent: Extent, packing: Packing) -> Result<Self, ImportError> {
        let row_bytes = u64::from(extent.width()) * u64::from(packing.bytes());
        let rows = (STRIPE_BYTES / row_bytes).min(u64::from(TILE_EDGE)) as u32;
        if rows == 0 {
            return Err(ImportError::Invalid("Image scanline exceeds bounded import workspace"));
        }
        let columns = extent.width().div_ceil(TILE_EDGE);
        let stripes = extent.height().div_ceil(rows);
        let length = extent
            .pixels()
            .checked_mul(u64::from(packing.bytes()))
            .and_then(|bytes| {
                bytes.checked_add(u64::from(columns) * u64::from(stripes) * HASH_BYTES)
            })
            .and_then(|bytes| bytes.checked_add(HEADER_BYTES))
            .filter(|length| *length <= MAX_FILE_BYTES)
            .ok_or(ImportError::Invalid("Image archive exceeds its storage-format limit"))?;
        Ok(Self { extent, packing, rows, columns, stripes, length })
    }
    /// Original source pixel dimensions.
    pub const fn extent(self) -> Extent {
        self.extent
    }
    /// Original integer sample representation.
    pub const fn packing(self) -> Packing {
        self.packing
    }
    /// Complete exact archive bytes, including header and independent chunk integrity hashes.
    pub const fn length(self) -> u64 {
        self.length
    }
    /// Maximum sequential-import allocation for the source pixel stripe, in bytes.
    pub fn stripe_bytes(self) -> usize {
        self.extent.width() as usize
            * self.rows.min(self.extent.height()) as usize
            * self.packing.bytes() as usize
    }
    /// Pixel rows in a complete chunk stripe, shortened automatically for wide sources.
    pub const fn rows(self) -> u32 {
        self.rows
    }
    /// Number of chunk columns.
    pub const fn columns(self) -> u32 {
        self.columns
    }
    /// Number of sequential chunk stripes.
    pub const fn stripes(self) -> u32 {
        self.stripes
    }
    /// Exact source rectangle for one chunk, clipping partial right and bottom edges.
    /// # Errors
    /// Rejects chunk indices outside this original extent.
    pub fn region(self, column: u32, stripe: u32) -> Result<Region, ImportError> {
        if column >= self.columns || stripe >= self.stripes {
            return Err(ImportError::Invalid("Image chunk lies outside source extent"));
        }
        let x = column * TILE_EDGE;
        let y = stripe * self.rows;
        Ok(Region::new(
            x,
            y,
            TILE_EDGE.min(self.extent.width() - x),
            self.rows.min(self.extent.height() - y),
        )?)
    }
    /// Offset of the chunk's SHA-256 followed immediately by its packed sample payload.
    /// # Errors
    /// Rejects chunk indices outside the original extent.
    pub fn offset(self, column: u32, stripe: u32) -> Result<u64, ImportError> {
        let chunk = self.region(column, stripe)?;
        let preceding_rows =
            u64::from(chunk.y()) * u64::from(self.extent.width()) * u64::from(self.packing.bytes());
        let preceding_chunks = u64::from(stripe) * u64::from(self.columns) + u64::from(column);
        let preceding_columns =
            u64::from(chunk.x()) * u64::from(chunk.height()) * u64::from(self.packing.bytes());
        Ok(HEADER_BYTES + preceding_rows + preceding_chunks * HASH_BYTES + preceding_columns)
    }
}
