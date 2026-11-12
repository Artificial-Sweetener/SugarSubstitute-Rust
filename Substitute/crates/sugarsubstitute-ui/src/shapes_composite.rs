// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Resume one independently rounded source-over silhouette pass.

use super::{AllocationError, CoverageRaster, blend_pixel, byte_product};
use crate::bitmap::{Bitmap, CoverageMask};
use tiny_skia::{Path, Rect};

/// Retain row/column position without changing any byte composition arithmetic.
pub(crate) struct CompositeCursor {
    left: u32,
    right: u32,
    row: u32,
    bottom: u32,
    column: u32,
}
impl CompositeCursor {
    /// Bound one pass by both its physical silhouette and optional widget clip.
    pub(crate) fn new(path: &Path, width: u32, height: u32, clip: Option<Rect>) -> Self {
        let bounds = clip.map_or(Some(path.bounds()), |clip| path.bounds().intersect(&clip));
        let (left, right, row, bottom) = bounds.map_or((0, 0, 0, 0), |bounds| {
            (
                bounds.left().floor().max(0.0).min(width as f32) as u32,
                bounds.right().ceil().min(width as f32).max(0.0) as u32,
                bounds.top().floor().max(0.0).min(height as f32) as u32,
                bounds.bottom().ceil().min(height as f32).max(0.0) as u32,
            )
        });
        Self { left, right, row, bottom, column: left }
    }

    /// Composite at most `max` physical pixels, preserving row-major pass order.
    pub(crate) fn step(
        &mut self,
        target: &mut Bitmap,
        color: [u8; 4],
        outer: Option<&CoverageMask>,
        mask: &CoverageMask,
        max: usize,
    ) -> bool {
        if self.left >= self.right {
            return true;
        }
        let mut remaining = max.clamp(1, 65536);
        while self.row < self.bottom && remaining > 0 {
            let end = self.right.min(self.column.saturating_add(remaining as u32));
            for column in self.column..end {
                let index = (self.row * target.width() + column) as usize;
                let coverage = outer.map_or(mask.data()[index], |outer| {
                    byte_product(mask.data()[index], outer.data()[index])
                });
                blend_pixel(target, i64::from(column), i64::from(self.row), color, coverage);
            }
            remaining -= (end - self.column) as usize;
            self.column = end;
            if end == self.right {
                self.column = self.left;
                self.row += 1;
            }
        }
        self.row >= self.bottom
    }
}

/// Share bounded coverage preparation and composition with synchronous callers.
pub struct PaintCursor {
    width: u32,
    height: u32,
    raster: Option<CoverageRaster>,
    mask: Option<CoverageMask>,
    composite: CompositeCursor,
    color: [u8; 4],
}
impl PaintCursor {
    /// Reserve one convex fill or stroke-outline plane and retain its widget clip.
    pub fn new(
        path: &Path,
        width: u32,
        height: u32,
        color: [u8; 4],
        clip: Option<Rect>,
        convex: bool,
    ) -> Result<Self, AllocationError> {
        Ok(Self {
            width,
            height,
            raster: Some(CoverageRaster::new(path, width, height, convex)?),
            mask: None,
            composite: CompositeCursor::new(path, width, height, clip),
            color,
        })
    }

    /// Advance one bounded coverage or composition slice and report pass completion.
    pub fn step(
        &mut self,
        target: &mut Bitmap,
        outer: Option<&CoverageMask>,
        max: usize,
    ) -> Result<bool, AllocationError> {
        if target.width() != self.width
            || target.height() != self.height
            || outer.is_some_and(|mask| mask.width() != self.width || mask.height() != self.height)
        {
            return Err(AllocationError::InvalidDimensions);
        }
        if let Some(raster) = &mut self.raster {
            if raster.step(max) {
                let raster = self.raster.take().ok_or(AllocationError::InvalidDimensions)?;
                self.mask = Some(raster.finish()?);
            }
            return Ok(false);
        }
        let mask = self.mask.as_ref().ok_or(AllocationError::InvalidDimensions)?;
        Ok(self.composite.step(target, self.color, outer, mask, max))
    }
}
