// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shared finite Gaussian support and coefficients for CPU and GPU image evaluation.
use crate::{Extent, ImageError, Region};

/// Largest Gaussian standard deviation in input-image pixels for one bounded operation.
/// Each axis has at most a 72-pixel halo and 145 coefficients.
pub const MAX_GAUSSIAN_SIGMA: f32 = 24.0;
/// Maximum one-dimensional coefficient count, including both three-sigma tails and the centre.
pub const MAX_GAUSSIAN_TAPS: usize = 145;

/// Isotropic Gaussian blur of linear premultiplied RGBA, truncated at three standard deviations.
/// Extent is preserved and samples outside the image clamp to its nearest edge pixel.
/// Zero standard deviation is an exact identity. Coefficients are shared by both evaluators.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GaussianBlur {
    sigma: f32,
}
impl GaussianBlur {
    /// Admit a standard deviation in input-image pixels, in `[0, MAX_GAUSSIAN_SIGMA]`.
    /// # Errors
    /// Rejects negative, nonfinite and excessive support before any graph or pixel allocation.
    pub fn new(sigma: f32) -> Result<Self, ImageError> {
        if !sigma.is_finite() || !(0.0..=MAX_GAUSSIAN_SIGMA).contains(&sigma) {
            return Err(ImageError::Invalid("Invalid Gaussian blur standard deviation"));
        }
        Ok(Self { sigma: if sigma == 0.0 { 0.0 } else { sigma } })
    }
    /// Standard deviation in input-image pixels, with signed zero normalized.
    pub const fn sigma(self) -> f32 {
        self.sigma
    }
    /// Finite support on either side of a pixel, in input-image pixels.
    pub fn radius(self) -> u32 {
        (f64::from(self.sigma) * 3.0).ceil() as u32
    }
    /// Normalized symmetric coefficients, ordered from negative to positive support.
    /// Only the first `2 * radius() + 1` entries are used; the remaining entries are zero.
    /// Callers charge this bounded stack workspace to their original evaluation admission.
    pub fn weights(self) -> [f32; MAX_GAUSSIAN_TAPS] {
        let radius = self.radius() as usize;
        let mut weights = [0.0; MAX_GAUSSIAN_TAPS];
        if radius == 0 {
            weights[0] = 1.0;
            return weights;
        }
        let coefficient = |index: usize| {
            let distance = (index as f64 - radius as f64) / f64::from(self.sigma);
            (-0.5 * distance * distance).exp()
        };
        let sum: f64 = (0..=2 * radius).map(coefficient).sum();
        for (index, weight) in weights[..=2 * radius].iter_mut().enumerate() {
            *weight = (coefficient(index) / sum) as f32;
        }
        weights
    }
    /// Include the finite convolution halo, clipped to the unchanged source extent.
    pub(crate) fn expanded(self, region: Region, extent: Extent) -> Result<Region, ImageError> {
        let radius = self.radius();
        let left = region.x().saturating_sub(radius);
        let top = region.y().saturating_sub(radius);
        let right = region.right().saturating_add(radius).min(extent.width());
        let bottom = region.bottom().saturating_add(radius).min(extent.height());
        Region::new(left, top, right - left, bottom - top)
    }
}
