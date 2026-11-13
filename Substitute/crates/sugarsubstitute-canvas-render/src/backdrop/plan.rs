// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shared CPU/GPU crop geometry and globally phased presentation DAGs.
use crate::RenderError;
use sugarsubstitute_contracts::execution::MemoryLease;
use sugarsubstitute_image_ops::{
    AssetId, Extent, GaussianBlur, Graph, MAX_GAUSSIAN_SIGMA, NodeId, Operation, Pixel, Region,
    Sampling, TILE_EDGE,
};

const MAX_REDUCTION: u32 = 64;
const MIB: u64 = 1024 * 1024;

/// One rounded physical viewport-local bar. Bounds are `[left, top, width, height]` before clipping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackdropRegion {
    bounds: [f64; 4],
    corner_radius: f64,
    sigma: f32,
}
impl BackdropRegion {
    /// Validate finite positive bounds, nonnegative uniform radius and physical-pixel sigma.
    /// Radius follows CSS uniform-radius normalization. Sigma is bounded by the shared Gaussian
    /// maximum at the largest supported 64-fold reduction; zero requests no change.
    /// # Errors
    /// Rejects invalid geometry, nonfinite arithmetic and sigma beyond bounded evaluation support.
    pub fn new(bounds: [f64; 4], corner_radius: f64, sigma: f32) -> Result<Self, RenderError> {
        if bounds.iter().any(|value| !value.is_finite())
            || bounds[2] <= 0.0
            || bounds[3] <= 0.0
            || !(bounds[0] + bounds[2]).is_finite()
            || !(bounds[1] + bounds[3]).is_finite()
            || !corner_radius.is_finite()
            || corner_radius < 0.0
            || !sigma.is_finite()
            || !(0.0..=MAX_GAUSSIAN_SIGMA * MAX_REDUCTION as f32).contains(&sigma)
        {
            return Err(RenderError::Invalid("Invalid presentation backdrop geometry"));
        }
        Ok(Self {
            bounds,
            corner_radius: corner_radius.min(bounds[2] * 0.5).min(bounds[3] * 0.5),
            sigma,
        })
    }
    /// Original physical viewport-local `[left, top, width, height]` bounds before clipping.
    pub const fn bounds(&self) -> [f64; 4] {
        self.bounds
    }
    /// CSS-normalized uniform physical corner radius.
    pub const fn corner_radius(&self) -> f64 {
        self.corner_radius
    }
    /// Standard deviation in physical viewport pixels, before reduction.
    pub const fn sigma(&self) -> f32 {
        self.sigma
    }
}

/// Immutable local source crop and mask placement, shared byte-for-byte by both display backends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackdropCrop {
    region: BackdropRegion,
    crop: Region,
    inner: Region,
    reduction: u32,
    blur: GaussianBlur,
}

/// One bounded inventory with no proportional allocation. An empty inventory leaves pixels exact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackdropPlan {
    viewport: Extent,
    regions: [BackdropCrop; 2],
    count: usize,
    scratch: u64,
}

/// One graph and its binding points; raster pixels cover the entire local padded source crop.
pub struct BackdropGraph {
    /// Owned operation graph with execution-backed metadata.
    pub graph: Graph,
    /// Raster node bound to the clean encoded projection crop after decoding.
    pub raster: NodeId,
    /// Last reduction node, before the Gaussian.
    pub reduced: NodeId,
    /// Reconstructed crop-sized output. Evaluate only [`BackdropCrop::inner_region`].
    pub output: NodeId,
}

impl BackdropPlan {
    /// Plan at most two physical bars, retaining original mask bounds when a bar is clipped.
    /// Invisible and zero-sigma bars produce no work. Crops include the finite Gaussian support,
    /// every reduction/reconstruction kernel and pixel phase guard, aligned to the global grid.
    /// # Errors
    /// Rejects excessive inventories and dimensions whose admitted workspace cannot be represented.
    pub fn new(viewport: Extent, regions: &[BackdropRegion]) -> Result<Self, RenderError> {
        if regions.len() > 2 {
            return Err(RenderError::Invalid("Too many presentation backdrops"));
        }
        let empty = BackdropCrop {
            region: BackdropRegion::new([0.0, 0.0, 1.0, 1.0], 0.0, 0.0)?,
            crop: viewport.region(),
            inner: viewport.region(),
            reduction: 4,
            blur: GaussianBlur::new(0.0)?,
        };
        let mut plan = Self { viewport, regions: [empty; 2], count: 0, scratch: 0 };
        let mut captured = 0_u64;
        let mut retained = 0_u64;
        let mut depth = 2_u32;
        for region in regions {
            let Some(crop) = BackdropCrop::new(viewport, *region)? else {
                continue;
            };
            captured =
                captured
                    .checked_add(crop.crop.pixels().checked_mul(4).ok_or(RenderError::Invalid(
                        "Backdrop capture exceeds addressable memory",
                    ))?)
                    .ok_or(RenderError::Invalid("Backdrop captures exceed addressable memory"))?;
            depth = depth.max(crop.reduction.ilog2());
            // Crops share one evaluator lifetime in sequence; only their clean encoded sources
            // coexist. Retain the largest crop's DAG inventory without evicting its dependencies.
            let tile_bytes = u64::from(TILE_EDGE).pow(2) * size_of::<Pixel>() as u64 + 256;
            retained = retained.max(crop.retained_tiles() as u64 * tile_bytes);
            plan.regions[plan.count] = crop;
            plan.count += 1;
        }
        if plan.count != 0 {
            // Recursive 128-pixel DAG tiles retain gathered halos while descending the pyramid.
            // Complete retained tiles are charged separately from these gathered buffers,
            // encoder, coefficient/tap tables and graph metadata.
            plan.scratch = captured
                .checked_add(retained)
                .and_then(|bytes| {
                    bytes.checked_add(8 * MIB + u64::from(depth - 2) * 2 * MIB + 128 * 1024)
                })
                .ok_or(RenderError::Invalid("Backdrop workspace exceeds addressable memory"))?;
        }
        Ok(plan)
    }
    /// Physical display extent whose completed projection supplies these crops.
    pub const fn viewport(&self) -> Extent {
        self.viewport
    }
    /// Visible nonidentity bars in composition order, with at most two entries.
    pub fn regions(&self) -> &[BackdropCrop] {
        &self.regions[..self.count]
    }
    /// Conservative CPU job scratch admission in bytes, including all encoded captures.
    /// It excludes the preexisting retained full projection allocation. Sampled DAG tiles retain
    /// their explicitly funded claims; reconstruction still streams without another crop-sized
    /// output allocation. GPU adapters account their own native storage.
    pub const fn cpu_scratch_bytes(&self) -> u64 {
        self.scratch
    }
}

impl BackdropCrop {
    fn new(viewport: Extent, region: BackdropRegion) -> Result<Option<Self>, RenderError> {
        if region.sigma == 0.0 {
            return Ok(None);
        }
        let [left, top, width, height] = region.bounds;
        let x = left.floor().clamp(0.0, f64::from(viewport.width())) as u32;
        let y = top.floor().clamp(0.0, f64::from(viewport.height())) as u32;
        let right = (left + width).ceil().clamp(0.0, f64::from(viewport.width())) as u32;
        let bottom = (top + height).ceil().clamp(0.0, f64::from(viewport.height())) as u32;
        if right <= x || bottom <= y {
            return Ok(None);
        }
        let mut reduction = 4_u32;
        while region.sigma / reduction as f32 > MAX_GAUSSIAN_SIGMA {
            reduction *= 2;
        }
        let blur = GaussianBlur::new(region.sigma / reduction as f32)?;
        // Each 2x Lanczos reduction contributes 6 source pixels at its preceding level.
        // Reconstruction contributes 3 reduced pixels; two more cover centre/rounding phase.
        let padding = 6 * (reduction - 1) + reduction * (blur.radius() + 5);
        let crop_x = x.saturating_sub(padding) / reduction * reduction;
        let crop_y = y.saturating_sub(padding) / reduction * reduction;
        let crop_right = u64::from(right)
            .saturating_add(u64::from(padding))
            .div_ceil(u64::from(reduction))
            .saturating_mul(u64::from(reduction))
            .min(u64::from(viewport.width())) as u32;
        let crop_bottom = u64::from(bottom)
            .saturating_add(u64::from(padding))
            .div_ceil(u64::from(reduction))
            .saturating_mul(u64::from(reduction))
            .min(u64::from(viewport.height())) as u32;
        Ok(Some(Self {
            region,
            crop: Region::new(crop_x, crop_y, crop_right - crop_x, crop_bottom - crop_y)?,
            inner: Region::new(x - crop_x, y - crop_y, right - x, bottom - y)?,
            reduction,
            blur,
        }))
    }
    /// Complete clipped source ROI in physical viewport coordinates.
    pub const fn crop(&self) -> Region {
        self.crop
    }
    /// Reconstructed output ROI relative to the crop; pixels outside it remain unchanged.
    pub const fn inner_region(&self) -> Region {
        self.inner
    }
    /// Original unclipped physical floating mask bounds `[left, top, width, height]`.
    pub const fn bounds(&self) -> [f64; 4] {
        self.region.bounds
    }
    /// Original CSS-normalized uniform physical corner radius.
    pub const fn corner_radius(&self) -> f64 {
        self.region.corner_radius
    }
    /// Power-of-two reduction shared by CPU/GPU, always at least four for nonidentity blur.
    pub const fn reduction(&self) -> u32 {
        self.reduction
    }
    /// Reduced-image Gaussian, using the existing shared finite support and coefficients.
    pub const fn blur(&self) -> GaussianBlur {
        self.blur
    }
    /// Bounded retained tile inventory of this crop's complete local presentation DAG.
    /// Counts the raster, every explicit reduction, Gaussian and reconstructed output. The
    /// existing evaluator's 4096-entry ceiling bounds unusually large crops; normal bar crops
    /// keep all dependencies while their inner output streams in tile-sized fragments.
    pub fn retained_tiles(&self) -> usize {
        let count = |width: u32, height: u32| {
            u64::from(width.div_ceil(TILE_EDGE)) * u64::from(height.div_ceil(TILE_EDGE))
        };
        let (mut width, mut height) = (self.crop.width(), self.crop.height());
        let mut tiles = 2 * count(width, height); // Raster and reconstructed output.
        for _ in 0..self.reduction.ilog2() {
            width = width.div_ceil(2);
            height = height.div_ceil(2);
            tiles += count(width, height);
        }
        tiles += count(width, height); // Gaussian at the reduced extent.
        tiles.min(4096) as usize
    }
    /// Construct the exact local DAG. Explicit 2x stages and inverse reconstruction retain
    /// global pixel phase even at odd or clipped viewport edges; normalized resize ratios would
    /// change that phase. The caller supplies an immutable source identity and admitted memory.
    /// # Errors
    /// Preserves graph, memory and sampling validation failures.
    pub fn graph(
        &self,
        asset: AssetId,
        memory: &MemoryLease,
    ) -> Result<BackdropGraph, RenderError> {
        let mut graph = Graph::new(self.reduction.ilog2() as usize + 3, memory)?;
        let extent = Extent::new(self.crop.width(), self.crop.height())?;
        let raster = graph.add(Operation::Raster { asset, extent })?;
        let mut reduced = raster;
        let mut low = extent;
        for _ in 0..self.reduction.ilog2() {
            low = Extent::new(low.width().div_ceil(2), low.height().div_ceil(2))?;
            reduced = graph.add(Operation::Resample {
                input: reduced,
                extent: low,
                sampling: Sampling::new([2.0; 2], [0.0; 2])?,
            })?;
        }
        let blurred = graph.add(Operation::GaussianBlur { input: reduced, blur: self.blur })?;
        let output = graph.add(Operation::Resample {
            input: blurred,
            extent,
            sampling: Sampling::new([1.0 / f64::from(self.reduction); 2], [0.0; 2])?,
        })?;
        Ok(BackdropGraph { graph, raster, reduced, output })
    }
    /// Deterministic 4x4 subpixel coverage of the original rounded bar at one viewport pixel.
    /// Clipping never moves corner centres; both display backends use this mask definition.
    pub fn coverage(&self, x: u32, y: u32) -> f32 {
        let [left, top, width, height] = self.region.bounds;
        let right = left + width;
        let bottom = top + height;
        let radius = self.region.corner_radius;
        let mut covered = 0_u32;
        for sy in 0..4 {
            for sx in 0..4 {
                let px = f64::from(x) + (f64::from(sx) + 0.5) / 4.0;
                let py = f64::from(y) + (f64::from(sy) + 0.5) / 4.0;
                if px < left || px >= right || py < top || py >= bottom {
                    continue;
                }
                let cx = px.clamp(left + radius, right - radius);
                let cy = py.clamp(top + radius, bottom - radius);
                if (px - cx).powi(2) + (py - cy).powi(2) <= radius * radius {
                    covered += 1;
                }
            }
        }
        covered as f32 / 16.0
    }
}
