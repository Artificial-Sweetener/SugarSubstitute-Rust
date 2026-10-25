// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Demand-driven image operations independent of native surfaces and storage implementations.
//!
//! The graph owns image semantics, including pyramid resampling and spatial dependencies.
//! Evaluation runs inside admitted execution jobs; returned tiles retain their original memory
//! claims. Source adapters provide bounded regions and never acquire scheduling authority.
//! Raster values use linear-light sRGB primaries, floating-point premultiplied RGBA. Geometry
//! remains explicit until evaluated; source assets and document editing state are not caches.

mod buffer;
mod display;
mod error;
mod evaluate;
mod geometry;
mod graph;
mod pixel;
mod resample;
mod sampling;
mod spatial;

pub use buffer::{TILE_EDGE, Tile};
pub use display::SrgbEncoder;
pub use error::ImageError;
pub use evaluate::{EvaluationStats, Evaluator, RasterSource};
pub use geometry::{Extent, Region};
pub use graph::{AssetId, Graph, NodeId, Operation, Pyramid, VectorRectangle};
pub use pixel::Pixel;
pub use sampling::Sampling;

#[cfg(test)]
mod tests;
