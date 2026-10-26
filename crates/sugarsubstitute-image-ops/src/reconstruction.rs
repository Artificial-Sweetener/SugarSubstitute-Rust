// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Distinguish disposable sparse previews from canonical finished reconstruction.

use crate::{Filter, ImageError};

/// Raster reconstruction intent for a complete canvas graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reconstruction {
    /// Sample the original raster directly with nearest-neighbor reconstruction.
    ///
    /// Large reductions use bounded sparse source reads and never construct a reduction pyramid.
    Preview,
    /// Build canonical Lanczos reduction levels and apply the selected final reconstruction.
    Finished(Filter),
}

impl Reconstruction {
    /// Final reconstruction kernel applied at the viewport grid.
    pub const fn filter(self) -> Filter {
        match self {
            Self::Preview => Filter::Nearest,
            Self::Finished(filter) => filter,
        }
    }

    /// Whether raster compilation must use canonical Lanczos reduction levels.
    pub const fn uses_canonical_pyramid(self) -> bool {
        matches!(self, Self::Finished(_))
    }

    /// Whether this intent is a disposable direct-source preview.
    pub const fn is_preview(self) -> bool {
        matches!(self, Self::Preview)
    }

    /// Select finished display reconstruction from a validated physical camera zoom.
    ///
    /// Zooms strictly above two device pixels per document pixel preserve hard source-pixel
    /// boundaries with nearest-neighbor final reconstruction. At or below two, finished display
    /// uses Lanczos-3 after canonical pyramid reduction.
    /// # Errors
    /// Rejects nonfinite or nonpositive zoom values.
    pub fn finished_for_display_zoom(zoom: f64) -> Result<Self, ImageError> {
        if !zoom.is_finite() || zoom <= 0.0 {
            return Err(ImageError::Invalid("Invalid display zoom"));
        }
        Ok(Self::Finished(if zoom > 2.0 { Filter::Nearest } else { Filter::Lanczos3 }))
    }
    /// Reuse canonical Lanczos pyramid levels with inexpensive bilinear navigation sampling.
    /// Physical zooms strictly above two retain nearest-neighbor pixel inspection.
    /// # Errors
    /// Rejects nonfinite or nonpositive physical zoom values.
    pub fn navigation_for_display_zoom(zoom: f64) -> Result<Self, ImageError> {
        let finished = Self::finished_for_display_zoom(zoom)?;
        Ok(if finished.filter() == Filter::Nearest {
            finished
        } else {
            Self::Finished(Filter::Bilinear)
        })
    }
}
