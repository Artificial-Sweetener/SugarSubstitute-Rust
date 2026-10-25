// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Linear-light floating point color with explicit premultiplied coverage.
use crate::ImageError;

/// Linear sRGB-primary RGBA with premultiplied RGB; negative/HDR color is preserved.
/// Alpha is finite coverage in `[0, 1]`; zero coverage carries zero color.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pixel(pub(crate) [f32; 4]);
impl Pixel {
    /// Transparent black.
    pub const TRANSPARENT: Self = Self([0.0; 4]);
    /// Convert finite straight linear-light color and coverage to canonical pixels.
    /// # Errors
    /// Rejects nonfinite channels and coverage outside `[0, 1]`.
    pub fn linear(red: f32, green: f32, blue: f32, alpha: f32) -> Result<Self, ImageError> {
        Self::premultiplied([red * alpha, green * alpha, blue * alpha, alpha])
    }
    /// Validate premultiplied data crossing an image adapter boundary.
    /// # Errors
    /// Rejects nonfinite channels, invalid alpha or colored zero-coverage pixels.
    pub fn premultiplied(channels: [f32; 4]) -> Result<Self, ImageError> {
        if channels.iter().any(|v| !v.is_finite())
            || !(0.0..=1.0).contains(&channels[3])
            || (channels[3] == 0.0 && channels[..3].iter().any(|v| *v != 0.0))
        {
            return Err(ImageError::Invalid("Invalid linear premultiplied pixel"));
        }
        Ok(Self(channels))
    }
    /// Borrow RGBA in canonical channel order, without color-space conversion.
    pub const fn channels(self) -> [f32; 4] {
        self.0
    }
    /// Porter-Duff source-over in linear light.
    /// # Errors
    /// Rejects floating-point overflow so canonical pixels remain finite across safe API calls.
    pub fn over(self, background: Self) -> Result<Self, ImageError> {
        let remaining = 1.0 - self.0[3];
        Self::premultiplied(std::array::from_fn(|i| self.0[i] + background.0[i] * remaining))
    }
    pub(crate) fn filtered(channels: [f32; 4]) -> Result<Self, ImageError> {
        let alpha = channels[3].clamp(0.0, 1.0);
        if alpha == 0.0 {
            return Ok(Self::TRANSPARENT);
        }
        let ratio = alpha / channels[3];
        Self::premultiplied([channels[0] * ratio, channels[1] * ratio, channels[2] * ratio, alpha])
    }
}
