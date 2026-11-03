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
mod coverage;
mod display;
mod display_sampling;
mod error;
mod evaluate;
mod geometry;
mod graph;
mod kernel;
mod pixel;
mod raster_samples;
mod raster_source;
mod reconstruction;
mod resample;
mod retention;
mod sampling;
mod spatial;

pub use buffer::{TILE_EDGE, Tile};
pub use coverage::{
    CombineMode, Coverage, CoverageAffine, CoverageBuilder, CoverageResampling, CoverageTileValues,
    FULL_COVERAGE, Path, coverage_from_u8, coverage_to_u8,
};
pub use display::SrgbEncoder;
pub use display_sampling::{DisplaySample, bilinear_display_pixel, bilinear_display_row};
pub use error::ImageError;
pub(crate) use error::check;
pub use evaluate::{EvaluationStats, Evaluator};
pub use geometry::{Extent, Region, RegionTiles};
pub use graph::{
    AssetId, CoverageSource, Graph, NodeId, Operation, Pyramid, SparseSourcePlan, VectorRectangle,
};
pub use kernel::{AxisKernel, MAX_SAMPLING_TAPS, SampleTap, SamplingAxis};
pub use pixel::Pixel;
pub use raster_samples::RasterSamples;
pub use raster_source::{
    MAX_RASTER_SAMPLE_BATCH, RasterSampleRequest, RasterSource, validate_raster_sample_batch,
};
pub use reconstruction::Reconstruction;
pub use retention::TileRetention;
pub use sampling::{Filter, Sampling};

#[cfg(test)]
mod tests;
