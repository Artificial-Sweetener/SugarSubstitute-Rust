// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Sample premultiplied card pixels with the reference raster's fixed-coordinate arithmetic.

use super::CharacterError;
use crate::projection::{Bounds, Point};
use tiny_skia::{FillRule, Mask, PathBuilder, Pixmap, Transform};

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
) -> Result<(), CharacterError> {
    let extent = f64::from(source.width());
    let sx = (quad[1].x - quad[0].x) / extent;
    let sy = (quad[3].y - quad[0].y) / extent;
    let ky = (quad[1].y - quad[0].y) / extent;
    let inverse_x = 1.0 / sx;
    let inverse_y = 1.0 / sy;
    let inverse_shear = -ky / (sx * sy);
    let step_x = (inverse_x * FIXED) as i64;
    let step_y = (inverse_shear * FIXED) as i64;
    let sheared = step_y != 0;
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
    outline.move_to(quad[0].x as f32, quad[0].y as f32);
    for point in &quad[1..] {
        outline.line_to(point.x as f32, point.y as f32);
    }
    outline.close();
    let path = outline.finish().ok_or(CharacterError::InvalidBounds)?;
    let mut coverage =
        Mask::new(target.width(), target.height()).ok_or(CharacterError::RasterAllocation)?;
    coverage.fill_path(&path, FillRule::Winding, true, Transform::identity());
    let source_width = source.width() as usize;
    let source_height = source.height() as usize;
    let pixels = target.data_mut();
    for y in y0..y1 {
        let row_coverage = &coverage.data()[y * width..(y + 1) * width];
        let Some(first) =
            row_coverage[x0..x1].iter().position(|alpha| *alpha != 0).map(|offset| x0 + offset)
        else {
            continue;
        };
        let last = row_coverage[x0..x1]
            .iter()
            .rposition(|alpha| *alpha != 0)
            .map_or(first, |offset| x0 + offset + 1);
        let center_x = first as f64 + 0.5 - quad[0].x;
        let center_y = y as f64 + 0.5 - quad[0].y;
        let mut fx = (((center_x * inverse_x - 1.0 / FIXED) * FIXED) as i64).saturating_sub(32_768);
        let mut fy = (((center_y * inverse_y + center_x * inverse_shear - 1.0 / FIXED) * FIXED)
            as i64)
            .saturating_sub(32_768);
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
        for (x, coverage) in row_coverage.iter().enumerate().take(last).skip(first) {
            let coarse = sheared && x >= precise_until && x < coarse_end;
            let sample = sample(source, fx, fy, coarse);
            let offset = (y * width + x) * 4;
            over(&mut pixels[offset..offset + 4], sample, *coverage);
            fx = fx.saturating_add(step_x);
            fy = fy.saturating_add(step_y);
        }
    }
    Ok(())
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
mod tests {
    use super::{bounded_count, circle_above, multiply, over, paint, paper_edge, sample};
    use crate::projection::{Bounds, Point};
    use std::error::Error;
    use tiny_skia::Pixmap;

    /// Preserve alpha validity and the source raster's downward interpolation at fractions.
    #[test]
    fn precise_sampling_retains_premultiplied_channels() -> Result<(), Box<dyn Error>> {
        let mut source = Pixmap::new(2, 2).ok_or("Cannot allocate sample raster")?;
        source.data_mut().copy_from_slice(&[
            101, 24, 18, 255, 102, 34, 27, 255, 102, 84, 81, 255, 114, 153, 148, 255,
        ]);
        assert_eq!(sample(&source, 16_383, 49_151, false), [103, 81, 77, 255]);
        assert_eq!(sample(&source, -32_768, -32_768, false), [101, 24, 18, 255]);
        assert_eq!(sample(&source, i64::MAX, i64::MAX, false), [114, 153, 148, 255]);
        Ok(())
    }

    /// Bound alpha multiplication and compose through actual pixel coverage.
    #[test]
    fn premultiplied_over_preserves_source_and_destination_limits() {
        assert_eq!(multiply(255, 255), 255);
        assert_eq!(multiply(255, 0), 0);
        assert_eq!(multiply(191, 253), 189);
        assert_eq!(multiply(226, 233), 206);
        let mut destination = [30, 40, 50, 255];
        over(&mut destination, [10, 20, 30, 128], 0);
        assert_eq!(destination, [30, 40, 50, 255]);
        over(&mut destination, [10, 20, 30, 128], 255);
        assert_eq!(destination, [25, 40, 55, 255]);
    }

    /// Preserve fixed-coordinate end bounds, including stationary and reversed sampling.
    #[test]
    fn complete_interpolation_steps_have_explicit_end_bounds() {
        assert_eq!(bounded_count(32_768, 65_536, 3), 1);
        assert_eq!(bounded_count(65_536, -65_536, 3), 1);
        assert_eq!(bounded_count(0, 0, 3), usize::MAX);
    }

    /// Retain symmetric quarter-pixel coverage and the original widget's physical clip.
    #[test]
    fn paper_edge_covers_fractional_columns_and_clips_widget_rows() -> Result<(), Box<dyn Error>> {
        let mut target = Pixmap::new(12, 12).ok_or("Cannot allocate edge raster")?;
        paper_edge(
            &mut target,
            Point { x: 5.0, y: 1.0 },
            Point { x: 5.0, y: 10.0 },
            2.5,
            89,
            Bounds { x: 0.0, y: 3.5, width: 12.0, height: 8.0 },
        );
        for x in [3, 6] {
            let offset = (6 * 12 + x) * 4;
            assert_eq!(&target.data()[offset..offset + 4], &[5, 2, 2, 22]);
        }
        assert_eq!(&target.data()[3 * 12 * 4..4 * 12 * 4], &[0; 48]);
        assert!((circle_above(-1.0, 1.0, 1.0, 0.0) - std::f64::consts::PI / 2.0).abs() < 1e-12);
        assert_eq!(circle_above(-1.0, 1.0, 1.0, 1.0), 0.0);
        Ok(())
    }

    /// Clip opaque airborne source pixels to the same widget rectangle as the paper edge.
    #[test]
    fn transformed_card_cannot_paint_outside_its_widget() -> Result<(), Box<dyn Error>> {
        let mut source = Pixmap::new(4, 4).ok_or("Cannot allocate source raster")?;
        source.fill(tiny_skia::Color::WHITE);
        let quad = [
            Point { x: 3.0, y: 1.0 },
            Point { x: 7.0, y: 0.0 },
            Point { x: 7.0, y: 8.0 },
            Point { x: 3.0, y: 9.0 },
        ];
        let mut target = Pixmap::new(12, 12).ok_or("Cannot allocate target raster")?;
        paint(&mut target, &source, quad, Bounds { x: 2.0, y: 3.0, width: 6.0, height: 4.0 })?;
        assert!(target.pixel(5, 4).is_some_and(|pixel| pixel.alpha() == 255));
        for y in 0..12 {
            for x in 0..12 {
                if !(2..8).contains(&x) || !(3..7).contains(&y) {
                    assert_eq!(target.pixel(x, y).map(|pixel| pixel.alpha()), Some(0));
                }
            }
        }
        paint(&mut target, &source, quad, Bounds { x: 20.0, y: 20.0, width: 1.0, height: 1.0 })?;
        Ok(())
    }

    /// Match the observed Qt widget clip at quarter- and three-quarter-pixel DPI origins.
    #[test]
    fn fractional_widget_origins_snap_once_to_device_pixels() -> Result<(), Box<dyn Error>> {
        for (origin, first_row) in [(8.75, 9), (12.25, 12)] {
            let mut target = Pixmap::new(20, 20).ok_or("Cannot allocate edge raster")?;
            paper_edge(
                &mut target,
                Point { x: 10.0, y: 1.0 },
                Point { x: 10.0, y: 18.0 },
                2.0,
                89,
                Bounds { x: 0.0, y: origin, width: 20.0, height: 20.0 },
            );
            assert_eq!(target.pixel(10, first_row - 1).map(|pixel| pixel.alpha()), Some(0));
            assert_eq!(target.pixel(10, first_row).map(|pixel| pixel.alpha()), Some(89));
        }
        Ok(())
    }
}
