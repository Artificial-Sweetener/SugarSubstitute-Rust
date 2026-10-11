// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Sample premultiplied card pixels with the reference raster's fixed-coordinate arithmetic.

use super::CharacterError;
use crate::projection::{Bounds, Point};
use tiny_skia::{PathBuilder, Pixmap};

#[path = "character_transform.rs"]
mod transform;
pub(super) use transform::DeviceTransform;

const FIXED: f64 = 65_536.0;

/// Prepare original RGBA PNG bytes with the reference renderer's integer multiplication order.
pub(super) fn premultiply_rgba(bytes: &mut [u8]) {
    for pixel in bytes.chunks_exact_mut(4) {
        let alpha = pixel[3];
        for channel in &mut pixel[..3] {
            *channel = multiply(*channel, alpha);
        }
    }
}

/// Integrate a vertical round-capped edge directly instead of snapping a narrow vector stroke.
pub(super) fn paper_edge(
    target: &mut Pixmap,
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

/// Paint an affine card with bounded coordinates and deterministic integer color arithmetic.
pub(super) fn paint(
    target: &mut Pixmap,
    source: &Pixmap,
    quad: [Point; 4],
    bounds: Bounds,
    mapping: DeviceTransform,
) -> Result<(), CharacterError> {
    let inverse = mapping.inverse();
    let width = target.width() as usize;
    let height = target.height() as usize;
    let x0 = quad[0].x.floor().max(bounds.x.round()).clamp(0.0, width as f64) as usize;
    let x1 =
        quad[1].x.ceil().min((bounds.x + bounds.width).round()).clamp(0.0, width as f64) as usize;
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
        return Ok(());
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
    let coverage = crate::shapes::coverage(&path, target.width(), target.height())
        .map_err(|source| CharacterError::Coverage { source: Box::new(source) })?;
    let pixels = target.data_mut();
    let mut span_count = 0;
    for y in y0..y1 {
        let row_coverage = &coverage.data()[y * width..(y + 1) * width];
        let mut fetch_start = None;
        let mut first = x0;
        while first < x1 {
            let alpha = row_coverage[first];
            let mut last = first + 1;
            while last < x1 && row_coverage[last] == alpha {
                last += 1;
            }
            if alpha != 0 {
                let start = *fetch_start.get_or_insert(first);
                span_count += 1;
                if span_count == 256 {
                    paint_span(pixels, source, inverse, width, y, start..last, row_coverage);
                    fetch_start = None;
                    span_count = 0;
                }
            } else if let Some(start) = fetch_start.take() {
                paint_span(pixels, source, inverse, width, y, start..first, row_coverage);
            }
            first = last;
        }
        if let Some(start) = fetch_start {
            paint_span(pixels, source, inverse, width, y, start..x1, row_coverage);
        }
    }
    Ok(())
}

/// Sample adjacent coverage spans together until the native 256-span callback buffer flushes.
fn paint_span(
    pixels: &mut [u8],
    source: &Pixmap,
    inverse: DeviceTransform,
    width: usize,
    y: usize,
    range: std::ops::Range<usize>,
    coverage: &[u8],
) {
    let source_width = source.width() as usize;
    let source_height = source.height() as usize;
    let step_x = (inverse.x_axis.x * FIXED) as i64;
    let step_y = (inverse.x_axis.y * FIXED) as i64;
    let sheared = step_y != 0;
    let first = range.start;
    let last = range.end;
    let source_center = inverse.map(first as f64 + 0.5, y as f64 + 0.5);
    let mut fx = ((source_center.x * FIXED) as i64).saturating_sub(32_768);
    let mut fy = ((source_center.y * FIXED) as i64).saturating_sub(32_768);
    let mut precise_until = first;
    while precise_until < last {
        let sample_x =
            fx.saturating_add(step_x.saturating_mul((precise_until - first) as i64)) >> 16;
        let sample_y =
            fy.saturating_add(step_y.saturating_mul((precise_until - first) as i64)) >> 16;
        if sample_x >= 0
            && sample_x + 1 < source_width as i64
            && sample_y >= 0
            && sample_y + 1 < source_height as i64
        {
            break;
        }
        precise_until += 1;
    }
    let start_fx = fx.saturating_add(step_x.saturating_mul((precise_until - first) as i64));
    let start_fy = fy.saturating_add(step_y.saturating_mul((precise_until - first) as i64));
    let x_count = bounded_count(start_fx, step_x, source_width);
    let y_count = bounded_count(start_fy, step_y, source_height);
    let bounded_end = last.min(precise_until.saturating_add(x_count.min(y_count)));
    let coarse_end = precise_until + (bounded_end - precise_until) / 8 * 8;
    for (x, coverage) in coverage.iter().enumerate().take(last).skip(first) {
        let coarse = sheared && x >= precise_until && x < coarse_end;
        let sample = sample(source, fx, fy, coarse);
        let offset = (y * width + x) * 4;
        over(&mut pixels[offset..offset + 4], sample, *coverage);
        fx = fx.saturating_add(step_x);
        fy = fy.saturating_add(step_y);
    }
}

/// Bound complete interpolated steps without including a partial final source interval.
fn bounded_count(start: i64, step: i64, extent: usize) -> usize {
    if step > 0 {
        ((((extent - 1) as i64) << 16).saturating_sub(start) / step).max(0) as usize
    } else if step < 0 {
        (start / step.saturating_neg()).max(0) as usize
    } else {
        usize::MAX
    }
}

/// Interpolate either coarse combined weights or precise vertical-then-horizontal weights.
fn sample(source: &Pixmap, fx: i64, fy: i64, coarse: bool) -> [u8; 4] {
    let ix = fx >> 16;
    let iy = fy >> 16;
    let x0 = ix.clamp(0, i64::from(source.width()) - 1) as usize;
    let x1 = (ix + 1).clamp(0, i64::from(source.width()) - 1) as usize;
    let y0 = iy.clamp(0, i64::from(source.height()) - 1) as usize;
    let y1 = (iy + 1).clamp(0, i64::from(source.height()) - 1) as usize;
    let width = source.width() as usize;
    let positions = [
        (y0 * width + x0) * 4,
        (y0 * width + x1) * 4,
        (y1 * width + x0) * 4,
        (y1 * width + x1) * 4,
    ];
    let mut output = [0; 4];
    let (divisor, ax, ay) = if coarse {
        (16, (((fx & 0xffff) >> 8) + 8) >> 4, (((fy & 0xffff) >> 8) + 8) >> 4)
    } else {
        (256, (fx & 0xffff) >> 8, (fy & 0xffff) >> 8)
    };
    for (channel, value) in output.iter_mut().enumerate() {
        let [top_left, top_right, bottom_left, bottom_right] =
            positions.map(|position| i64::from(source.data()[position + channel]));
        let result = if coarse {
            (top_left * (divisor - ax) * (divisor - ay)
                + top_right * ax * (divisor - ay)
                + bottom_left * (divisor - ax) * ay
                + bottom_right * ax * ay)
                / (divisor * divisor)
        } else {
            let left = (top_left * (divisor - ay) + bottom_left * ay) / divisor;
            let right = (top_right * (divisor - ay) + bottom_right * ay) / divisor;
            (left * (divisor - ax) + right * ax) / divisor
        };
        *value = result as u8;
    }
    output
}

/// Compose premultiplied source over destination with separately bounded path coverage.
fn over(destination: &mut [u8], source: [u8; 4], coverage: u8) {
    let source = source.map(|value| multiply(value, coverage));
    for (channel, value) in destination.iter_mut().enumerate() {
        *value = source[channel].saturating_add(multiply(*value, 255 - source[3]));
    }
}

/// Round a byte product divided by 255 without floating-point color conversion.
fn multiply(value: u8, alpha: u8) -> u8 {
    let product = u16::from(value) * u16::from(alpha);
    ((product + (product >> 8) + 128) >> 8) as u8
}

#[cfg(test)]
#[path = "character_sampling_tests.rs"]
mod tests;
