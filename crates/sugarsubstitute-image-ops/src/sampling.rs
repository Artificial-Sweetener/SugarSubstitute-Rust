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
}

/// Axis-aligned inverse mapping from output pixel corners to input pixel corners.
/// Pixel centers map as `(coordinate + 0.5) * scale + offset - 0.5` in input-index space.
/// Reconstruction clamps source edges. Finite image footprints are separate coverage nodes,
/// allowing a minified one-pixel pyramid axis to retain correct geometric coverage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sampling {
    scale: [f64; 2],
    offset: [f64; 2],
    filter: Filter,
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
        Ok(Self { scale, offset, filter: Filter::Lanczos3 })
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
        }
    }
    /// Inverse scales in input pixels per output pixel.
    pub const fn scale(self) -> [f64; 2] {
        self.scale
    }
    /// Input pixel-corner coordinates corresponding to output pixel corner zero.
    pub const fn offset(self) -> [f64; 2] {
        self.offset
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
    pub(crate) fn validate(self, source: Extent, target: Extent) -> Result<(), ImageError> {
        for (axis, (input, output)) in
            [(source.width(), target.width()), (source.height(), target.height())]
                .into_iter()
                .enumerate()
        {
            if input > 1 && self.scale[axis] > 2.0 {
                return Err(ImageError::Invalid("Large reductions require pyramid steps"));
            }
            if !(f64::from(output) * self.scale[axis] + self.offset[axis]).is_finite() {
                return Err(ImageError::Invalid("Image sampling coordinate overflow"));
            }
        }
        Ok(())
    }
}
