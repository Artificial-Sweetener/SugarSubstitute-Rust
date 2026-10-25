// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe sparse immutable source reads without delegating sampling mathematics to I/O adapters.
use crate::{Extent, Filter, ImageError, Region, Sampling, TILE_EDGE, kernel::nearest_coordinate};

/// Exact original-pixel indices for one bounded nearest-neighbor output region.
/// Coordinates are computed by the DAG; source adapters decode only these canonical pixels.
/// Positive scales make both index arrays nondecreasing, including repeated clamped edge samples.
pub struct RasterSamples {
    source: Extent,
    region: Region,
    columns: [u32; TILE_EDGE as usize],
    rows: [u32; TILE_EDGE as usize],
}
impl RasterSamples {
    /// Plan at most one output tile using the shared global nearest-pixel phase and edge rules.
    /// # Errors
    /// Rejects other filters, excessive output regions and nonfinite projected coordinates.
    pub fn new(source: Extent, region: Region, sampling: Sampling) -> Result<Self, ImageError> {
        if sampling.filter() != Filter::Nearest
            || region.width() > TILE_EDGE
            || region.height() > TILE_EDGE
            || !(f64::from(region.right()) * sampling.scale()[0] + sampling.offset()[0]).is_finite()
            || !(f64::from(region.bottom()) * sampling.scale()[1] + sampling.offset()[1])
                .is_finite()
        {
            return Err(ImageError::Invalid("Invalid sparse raster sampling request"));
        }
        let mut result = Self {
            source,
            region,
            columns: [0; TILE_EDGE as usize],
            rows: [0; TILE_EDGE as usize],
        };
        for (index, column) in result.columns[..region.width() as usize].iter_mut().enumerate() {
            *column = nearest_coordinate(
                region.x() + index as u32,
                source.width(),
                sampling.scale()[0],
                sampling.offset()[0],
            );
        }
        for (index, row) in result.rows[..region.height() as usize].iter_mut().enumerate() {
            *row = nearest_coordinate(
                region.y() + index as u32,
                source.height(),
                sampling.scale()[1],
                sampling.offset()[1],
            );
        }
        Ok(result)
    }
    /// Original source dimensions in image pixels.
    pub const fn source(&self) -> Extent {
        self.source
    }
    /// Global output region; destination pixels are row-major at this region's width.
    pub const fn region(&self) -> Region {
        self.region
    }
    /// Nondecreasing original source column indices, one per destination column.
    pub fn columns(&self) -> &[u32] {
        &self.columns[..self.region.width() as usize]
    }
    /// Nondecreasing original source row indices, one per destination row.
    pub fn rows(&self) -> &[u32] {
        &self.rows[..self.region.height() as usize]
    }
}
