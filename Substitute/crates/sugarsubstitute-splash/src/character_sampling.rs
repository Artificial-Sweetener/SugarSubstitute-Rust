// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Sample premultiplied card pixels with the reference raster's fixed-coordinate arithmetic.

use super::CharacterError;
use crate::projection::{Bounds, Point};
use sugarsubstitute_ui::bitmap::Bitmap;
use tiny_skia::PathBuilder;

#[path = "character_transform.rs"]
mod transform;
pub(super) use transform::DeviceTransform;

#[path = "character_sample.rs"]
mod sample;
use sample::{multiply, over, paint_span};

/// Integrate a vertical round-capped edge directly instead of snapping a narrow vector stroke.
pub(super) fn paper_edge(
    target: &mut Bitmap,
    top: Point,
    bottom: Point,
    thickness: f64,
    alpha: u8,
    bounds: Bounds,
) {
    let radius = thickness / 2.0;
    if radius <= 0.0 || alpha == 0 {
        return;
    }
    let width = target.width() as usize;
    let height = target.height() as usize;
    let x0 = (top.x - radius).floor().max(bounds.x.round()).clamp(0.0, width as f64) as usize;
    let x1 = (top.x + radius).ceil().min((bounds.x + bounds.width).round()).clamp(0.0, width as f64)
        as usize;
    let y0 = (top.y - radius).floor().max(bounds.y.round()).clamp(0.0, height as f64) as usize;
    let y1 = (bottom.y + radius)
        .ceil()
        .min((bounds.y + bounds.height).round())
        .clamp(0.0, height as f64) as usize;
    let source = [multiply(54, alpha), multiply(16, alpha), multiply(16, alpha), alpha];
    let pixels = target.data_mut();
    for y in y0..y1 {
        for x in x0..x1 {
            let left = (x as f64 - top.x).max(-radius);
            let right = (x as f64 + 1.0 - top.x).min(radius);
            let row_top = y as f64;
            let row_bottom = row_top + 1.0;
            let rectangle =
                (right - left).max(0.0) * (row_bottom.min(bottom.y) - row_top.max(top.y)).max(0.0);
            let upper = cap_area(
                left,
                right,
                radius,
                (top.y - row_bottom).max(0.0),
                (top.y - row_top).max(0.0),
            );
            let lower = cap_area(
                left,
                right,
                radius,
                (row_top - bottom.y).max(0.0),
                (row_bottom - bottom.y).max(0.0),
            );
            let coverage = ((rectangle + upper + lower) * 256.0 + 1e-9).clamp(0.0, 255.0) as u8;
            let offset = (y * width + x) * 4;
            over(&mut pixels[offset..offset + 4], source, coverage);
        }
    }
}

/// Compute the intersection of one circular cap and a pixel's outward vertical interval.
fn cap_area(left: f64, right: f64, radius: f64, near: f64, far: f64) -> f64 {
    circle_above(left, right, radius, near) - circle_above(left, right, radius, far)
}

/// Integrate the part of a circle above a horizontal threshold within a pixel column.
fn circle_above(left: f64, right: f64, radius: f64, threshold: f64) -> f64 {
    if threshold >= radius || left >= right {
        return 0.0;
    }
    let extent = (radius * radius - threshold * threshold).max(0.0).sqrt();
    let left = left.max(-extent);
    let right = right.min(extent);
    if left >= right {
        return 0.0;
    }
    circle_primitive(right, radius) - circle_primitive(left, radius) - threshold * (right - left)
}

/// Evaluate the integral of the upper semicircle without losing endpoints to rounding.
fn circle_primitive(x: f64, radius: f64) -> f64 {
    0.5 * (x * (radius * radius - x * x).max(0.0).sqrt()
        + radius * radius * (x / radius).clamp(-1.0, 1.0).asin())
}

/// Retain raster and sampler progress; the native 256-span count crosses row/yield boundaries.
pub(super) struct SamplingCursor {
    inverse: DeviceTransform,
    width: usize,
    x0: usize,
    x1: usize,
    row: usize,
    bottom: usize,
    span_count: usize,
    raster: Option<sugarsubstitute_ui::shapes::CoverageRaster>,
    coverage: Option<sugarsubstitute_ui::bitmap::CoverageMask>,
}
impl SamplingCursor {
    /// Capture bounded card geometry without initializing the full coverage plane.
    pub(super) fn new(
        width: usize,
        height: usize,
        quad: [Point; 4],
        bounds: Bounds,
        mapping: DeviceTransform,
    ) -> Result<Self, CharacterError> {
        let inverse = mapping.inverse();
        let x0 = quad[0].x.floor().max(bounds.x.round()).clamp(0.0, width as f64) as usize;
        let x1 = quad[1].x.ceil().min((bounds.x + bounds.width).round()).clamp(0.0, width as f64)
            as usize;
        let y0 = quad
            .iter()
            .map(|point| point.y)
            .fold(f64::INFINITY, f64::min)
            .floor()
            .max(bounds.y.round())
            .clamp(0.0, height as f64) as usize;
        let y1 = quad
            .iter()
            .map(|point| point.y)
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil()
            .min((bounds.y + bounds.height).round())
            .clamp(0.0, height as f64) as usize;
        if x0 >= x1 || y0 >= y1 {
            return Ok(Self {
                inverse,
                width,
                x0,
                x1,
                row: y1,
                bottom: y1,
                span_count: 0,
                raster: None,
                coverage: None,
            });
        }
        let mut outline = PathBuilder::new();
        let snapped = quad.map(|point| Point {
            x: (point.x * 64.0).round() / 64.0,
            y: (point.y * 64.0).round() / 64.0,
        });
        outline.move_to(snapped[0].x as f32, snapped[0].y as f32);
        for point in &snapped[1..] {
            outline.line_to(point.x as f32, point.y as f32);
        }
        outline.close();
        let path = outline.finish().ok_or(CharacterError::InvalidBounds)?;
        let raster = sugarsubstitute_ui::shapes::CoverageRaster::new(
            &path,
            width as u32,
            height as u32,
            true,
        )
        .map_err(coverage_error)?;
        Ok(Self {
            inverse,
            width,
            x0,
            x1,
            row: y0,
            bottom: y1,
            span_count: 0,
            raster: Some(raster),
            coverage: None,
        })
    }
    /// Initialize/integrate at most 32768 coverage pixels or sample at most eight rows.
    pub(super) fn step(
        &mut self,
        target: &mut Bitmap,
        source: &Bitmap,
    ) -> Result<bool, CharacterError> {
        if let Some(raster) = &mut self.raster {
            if raster.step(32768) {
                let raster = self.raster.take().ok_or(CharacterError::InvalidBounds)?;
                self.coverage = Some(raster.finish().map_err(coverage_error)?);
            }
            return Ok(false);
        }
        if self.row == self.bottom {
            return Ok(true);
        }
        let coverage = self.coverage.as_ref().ok_or(CharacterError::InvalidBounds)?;
        let pixels = target.data_mut();
        let end = (self.row + 8).min(self.bottom);
        for y in self.row..end {
            let row_coverage = &coverage.data()[y * self.width..(y + 1) * self.width];
            let mut fetch_start = None;
            let mut first = self.x0;
            while first < self.x1 {
                let alpha = row_coverage[first];
                let mut last = first + 1;
                while last < self.x1 && row_coverage[last] == alpha {
                    last += 1;
                }
                if alpha != 0 {
                    let start = *fetch_start.get_or_insert(first);
                    self.span_count += 1;
                    if self.span_count == 256 {
                        paint_span(
                            pixels,
                            source,
                            self.inverse,
                            self.width,
                            y,
                            start..last,
                            row_coverage,
                        );
                        fetch_start = None;
                        self.span_count = 0;
                    }
                } else if let Some(start) = fetch_start.take() {
                    paint_span(
                        pixels,
                        source,
                        self.inverse,
                        self.width,
                        y,
                        start..first,
                        row_coverage,
                    );
                }
                first = last;
            }
            if let Some(start) = fetch_start {
                paint_span(
                    pixels,
                    source,
                    self.inverse,
                    self.width,
                    y,
                    start..self.x1,
                    row_coverage,
                );
            }
        }

        self.row = end;
        Ok(self.row == self.bottom)
    }
}
/// Preserve the allocation owner cause across character preparation.
fn coverage_error(source: sugarsubstitute_ui::shapes::AllocationError) -> CharacterError {
    CharacterError::Coverage { source: Box::new(source) }
}
/// Drive the same bounded card stages for supported synchronous qualification callers.
#[cfg(test)]
pub(super) fn paint(
    target: &mut Bitmap,
    source: &Bitmap,
    quad: [Point; 4],
    bounds: Bounds,
    mapping: DeviceTransform,
) -> Result<(), CharacterError> {
    let mut cursor = SamplingCursor::new(
        target.width() as usize,
        target.height() as usize,
        quad,
        bounds,
        mapping,
    )?;
    while !cursor.step(target, source)? {}
    Ok(())
}

#[cfg(test)]
#[path = "character_sampling_tests.rs"]
mod tests;
