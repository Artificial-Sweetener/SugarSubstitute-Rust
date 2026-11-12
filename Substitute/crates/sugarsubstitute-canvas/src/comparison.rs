// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare a second raster against the authored document through a wipe divider.
//!
//! Comparison is inspection state, never authored content: the second raster is stretched into
//! the document rectangle under the same camera and revealed on the leading side of the divider.
//! The divider is a document-space edge so retained backings and their projections place it
//! exactly; callers derive it from their viewport-anchored wipe for each camera.
use crate::CanvasError;
use sugarsubstitute_image_ops::{AssetId, Extent};

/// Axis the wipe divider runs along.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WipeOrientation {
    /// A vertical divider; the comparison raster is revealed left of the boundary.
    Vertical,
    /// A horizontal divider; the comparison raster is revealed above the boundary.
    Horizontal,
}

/// One comparison raster and its divider in document coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Comparison {
    asset: AssetId,
    extent: Extent,
    orientation: WipeOrientation,
    boundary: f64,
}

impl Comparison {
    /// Describe the comparison raster's own extent and a finite document-space divider. The
    /// boundary is a document x for a vertical divider and a document y for a horizontal one;
    /// values outside the document reveal the whole raster or none of it.
    /// # Errors
    /// Rejects a non-finite boundary.
    pub fn new(
        asset: AssetId,
        extent: Extent,
        orientation: WipeOrientation,
        boundary: f64,
    ) -> Result<Self, CanvasError> {
        if !boundary.is_finite() {
            return Err(CanvasError::Invalid("Comparison boundary must be finite"));
        }
        Ok(Self { asset, extent, orientation, boundary })
    }

    /// Content identity of the comparison raster.
    pub const fn asset(self) -> AssetId {
        self.asset
    }

    /// The comparison raster's own pixel extent, stretched into the document rectangle.
    pub const fn extent(self) -> Extent {
        self.extent
    }

    /// Axis of the wipe divider.
    pub const fn orientation(self) -> WipeOrientation {
        self.orientation
    }

    /// Document-space divider coordinate along the axis the divider crosses.
    pub const fn boundary(self) -> f64 {
        self.boundary
    }
}
