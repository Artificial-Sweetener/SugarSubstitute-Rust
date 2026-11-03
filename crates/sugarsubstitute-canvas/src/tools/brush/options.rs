// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The brush settings a person chooses: size, hardness and opacity.
use crate::CanvasError;

/// Size in document pixels, hardness and opacity of the Brush and Eraser tools. Flow is fixed
/// at one and dab spacing at a fifth of the diameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushOptions {
    size: u32,
    hardness: f64,
    opacity: f64,
}

impl BrushOptions {
    /// Smallest brush diameter in document pixels.
    pub const MIN_SIZE: u32 = 1;
    /// Largest brush diameter in document pixels.
    pub const MAX_SIZE: u32 = 1000;
    /// Size change for one mouse-wheel notch, in document pixels.
    pub const WHEEL_STEP: u32 = 5;
    /// A fully hard, fully opaque 20 pixel brush.
    pub const DEFAULT: Self = Self { size: 20, hardness: 1.0, opacity: 1.0 };

    /// Validate brush settings: `size` in document pixels, `hardness` and `opacity` as
    /// fractions in `[0, 1]`.
    /// # Errors
    /// Rejects a size outside [`Self::MIN_SIZE`]..=[`Self::MAX_SIZE`] and a nonfinite or
    /// out-of-range hardness or opacity.
    pub fn new(size: u32, hardness: f64, opacity: f64) -> Result<Self, CanvasError> {
        if !(Self::MIN_SIZE..=Self::MAX_SIZE).contains(&size) {
            return Err(CanvasError::Invalid("Brush size is outside 1 to 1000 pixels"));
        }
        for fraction in [hardness, opacity] {
            if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
                return Err(CanvasError::Invalid("Brush hardness and opacity must be 0 to 1"));
            }
        }
        Ok(Self { size, hardness, opacity })
    }
    /// Diameter in document pixels at full pressure.
    pub const fn size(self) -> u32 {
        self.size
    }
    /// Fraction of the radius painted at full opacity before the edge falls off.
    pub const fn hardness(self) -> f64 {
        self.hardness
    }
    /// Peak coverage one dab adds.
    pub const fn opacity(self) -> f64 {
        self.opacity
    }
    /// The same settings with the size moved by whole wheel `notches`, clamped to the range.
    pub fn resized_by(self, notches: i64) -> Self {
        let size = i64::from(self.size)
            .saturating_add(notches.saturating_mul(i64::from(Self::WHEEL_STEP)))
            .clamp(i64::from(Self::MIN_SIZE), i64::from(Self::MAX_SIZE));
        Self { size: size as u32, ..self }
    }
}

impl Default for BrushOptions {
    fn default() -> Self {
        Self::DEFAULT
    }
}
