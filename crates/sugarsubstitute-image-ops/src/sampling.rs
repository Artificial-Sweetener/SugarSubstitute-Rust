// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Explicit inverse pixel mapping and reconstruction quality for bounded image operations.
use crate::{Extent, ImageError};

/// Reconstruction semantics included in graph content identities and dependency regions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Filter {
    /// Normalized antialiased Lanczos-3 for finished viewing and canonical pyramids.
    Lanczos3,
    /// Closest pixel center, with positive ties selecting the next pixel and edges clamped.
    Nearest,
    /// Two clamped linear taps per axis, for navigation over canonical pyramid pixels.
    Bilinear,
}

/// Axis-aligned inverse mapping from output pixel corners to input pixel corners.
/// Pixel centers map as `(coordinate - translation + 0.5) * scale + offset - 0.5`.
/// Integer translation is subtracted before floating-point conversion, preserving exact phases.
/// Reconstruction clamps source edges. Finite image footprints are separate coverage nodes,
/// allowing a minified one-pixel pyramid axis to retain correct geometric coverage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sampling {
    scale: [f64; 2],
    offset: [f64; 2],
    filter: Filter,
    translation: [i32; 2],
}
impl Sampling {
    /// Construct finite positive inverse scales and finite input-space offsets.
    /// # Errors
    /// Rejects nonfinite or nonpositive parameters. Graph admission separately bounds filter work.
    pub fn new(scale: [f64; 2], offset: [f64; 2]) -> Result<Self, ImageError> {
        if scale.iter().any(|v| !v.is_finite() || *v <= 0.0)
            || offset.iter().any(|v| !v.is_finite())
        {
            return Err(ImageError::Invalid("Invalid image sampling transform"));
        }
        Ok(Self { scale, offset, filter: Filter::Lanczos3, translation: [0; 2] })
    }
    /// Map a complete source extent into a complete target extent with aligned pixel corners.
    pub fn resize(source: Extent, target: Extent) -> Self {
        Self {
            scale: [
                f64::from(source.width()) / f64::from(target.width()),
                f64::from(source.height()) / f64::from(target.height()),
            ],
            offset: [0.0; 2],
            filter: Filter::Lanczos3,
            translation: [0; 2],
        }
    }
    /// Inverse scales in input pixels per output pixel.
    pub const fn scale(self) -> [f64; 2] {
        self.scale
    }
    /// Input pixel-corner coordinates corresponding to unshifted output pixel corner zero.
    pub const fn offset(self) -> [f64; 2] {
        self.offset
    }
    /// Move reconstructed content by exact output pixels while preserving its original sample phase.
    pub const fn with_output_translation(mut self, translation: [i32; 2]) -> Self {
        self.translation = translation;
        self
    }
    /// Exact output-pixel displacement; positive values move content right and down.
    pub const fn output_translation(self) -> [i32; 2] {
        self.translation
    }
    pub(crate) fn unshifted_index(self, axis: usize, output: u32) -> i64 {
        i64::from(output) - i64::from(self.translation[axis])
    }
    pub(crate) fn center(self, axis: usize, output: u32) -> f64 {
        crate::kernel::pixel_center(
            self.unshifted_index(axis, output),
            self.scale[axis],
            self.offset[axis],
        )
    }
    /// Select explicit reconstruction semantics while preserving global sample coordinates.
    pub const fn with_filter(mut self, filter: Filter) -> Self {
        self.filter = filter;
        self
    }
    /// Current reconstruction semantics; canonical resize defaults to Lanczos-3.
    pub const fn filter(self) -> Filter {
        self.filter
    }
    pub(crate) fn validate(
        self,
        source: Extent,
        target: Extent,
        sampled_raster: bool,
    ) -> Result<(), ImageError> {
        for (axis, (input, output)) in
            [(source.width(), target.width()), (source.height(), target.height())]
                .into_iter()
                .enumerate()
        {
            if input > 1 && self.scale[axis] > 2.0 && !sampled_raster {
                return Err(ImageError::Invalid("Large reductions require pyramid steps"));
            }
            if [0, output - 1].into_iter().any(|position| !self.center(axis, position).is_finite())
            {
                return Err(ImageError::Invalid("Image sampling coordinate overflow"));
            }
        }
        Ok(())
    }
    pub(crate) fn large_reduction(self, source: Extent) -> bool {
        (source.width() > 1 && self.scale[0] > 2.0) || (source.height() > 1 && self.scale[1] > 2.0)
    }
}
