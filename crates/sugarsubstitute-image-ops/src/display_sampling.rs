// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deterministic encoded-pixel sampling for disposable retained display projection.

/// One bounded display-axis sample using an unsigned Q8 interpolation weight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplaySample {
    first: u32,
    second: u32,
    weight: u32,
}
impl DisplaySample {
    /// Sentinel for an output position outside retained source coverage.
    pub const UNCOVERED: Self = Self { first: u32::MAX, second: u32::MAX, weight: 0 };

    /// Map a pixel-corner position to nearest or centered bilinear source pixels.
    ///
    /// Bilinear coordinates clamp their centered position to the covered edge. Nonfinite and
    /// outside corner positions remain uncovered instead of stretching the retained image.
    pub fn from_corner(position: f64, extent: u32, smooth: bool) -> Self {
        if !position.is_finite() || position < 0.0 || position >= f64::from(extent) {
            return Self::UNCOVERED;
        }
        if !smooth {
            let first = position.floor() as u32;
            return Self { first, second: first, weight: 0 };
        }
        let last = extent - 1;
        let centered = (position - 0.5).clamp(0.0, f64::from(last));
        let first = centered.floor() as u32;
        let second = first.saturating_add(1).min(last);
        let weight = ((centered - f64::from(first)) * 256.0).round() as u32;
        Self { first, second, weight }
    }

    /// First clamped source index, or `u32::MAX` when uncovered.
    pub const fn first(self) -> u32 {
        self.first
    }
    /// Second clamped source index, or `u32::MAX` when uncovered.
    pub const fn second(self) -> u32 {
        self.second
    }
    /// Unsigned Q8 contribution of the second sample, from zero through 256.
    pub const fn weight(self) -> u32 {
        self.weight
    }
    /// Whether this sample addresses retained source pixels.
    pub const fn is_covered(self) -> bool {
        self.first != u32::MAX
    }
    /// Whether this value is the uncovered sentinel or safely addresses the supplied extent.
    pub const fn is_valid_for(self, extent: u32) -> bool {
        (self.first == u32::MAX && self.second == u32::MAX && self.weight == 0)
            || (self.first < extent && self.second < extent && self.weight <= 256)
    }
}

/// Interpolate four opaque encoded `0xffRRGGBB` pixels with one rounded Q16 accumulation.
///
/// Pixels are ordered top-left, top-right, bottom-left, bottom-right. This is disposable display
/// interpolation; canonical linear-light image reconstruction remains in the operation graph.
pub fn bilinear_display_pixel(
    pixels: [u32; 4],
    horizontal: DisplaySample,
    vertical: DisplaySample,
) -> u32 {
    let right = u64::from(horizontal.weight);
    let left = 256 - right;
    let bottom = u64::from(vertical.weight);
    let top = 256 - bottom;
    let weights = [left * top, right * top, left * bottom, right * bottom];
    let channel = |shift: u32| {
        let sum = pixels
            .into_iter()
            .zip(weights)
            .map(|(pixel, weight)| u64::from((pixel >> shift) & 0xff_u32) * weight)
            .sum::<u64>();
        ((sum + 32_768) >> 16) as u32
    };
    0xff00_0000 | (channel(16) << 16) | (channel(8) << 8) | channel(0)
}
