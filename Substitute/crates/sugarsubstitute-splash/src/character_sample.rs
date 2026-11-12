// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Preserve the reference sampler integer arithmetic within a bounded row.

use super::DeviceTransform;
use sugarsubstitute_ui::bitmap::Bitmap;
const FIXED: f64 = 65_536.0;
/// Sample adjacent coverage spans together until the native 256-span callback buffer flushes.
pub(super) fn paint_span(
    pixels: &mut [u8],
    source: &Bitmap,
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
pub(super) fn bounded_count(start: i64, step: i64, extent: usize) -> usize {
    if step > 0 {
        ((((extent - 1) as i64) << 16).saturating_sub(start) / step).max(0) as usize
    } else if step < 0 {
        (start / step.saturating_neg()).max(0) as usize
    } else {
        usize::MAX
    }
}

/// Interpolate either coarse combined weights or precise vertical-then-horizontal weights.
pub(super) fn sample(source: &Bitmap, fx: i64, fy: i64, coarse: bool) -> [u8; 4] {
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
pub(super) fn over(destination: &mut [u8], source: [u8; 4], coverage: u8) {
    let source = source.map(|value| multiply(value, coverage));
    for (channel, value) in destination.iter_mut().enumerate() {
        *value = source[channel].saturating_add(multiply(*value, 255 - source[3]));
    }
}

/// Round a byte product divided by 255 without floating-point color conversion.
pub(super) fn multiply(value: u8, alpha: u8) -> u8 {
    let product = u16::from(value) * u16::from(alpha);
    ((product + (product >> 8) + 128) >> 8) as u8
}
