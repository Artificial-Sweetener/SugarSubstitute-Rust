// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Append-only typed DAGs with structural identities and bounded graph ownership.
use crate::{Extent, ImageError, Pixel, Sampling};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};

static NEXT_GRAPH: AtomicU64 = AtomicU64::new(1);
const MAX_NODES: usize = 4096;
const MAX_DEPTH: u16 = 128;

/// Immutable source-content identity supplied by its asset owner, never a path alone.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AssetId(pub [u8; 32]);
/// One node in exactly one process-local graph; graph snapshots cannot exchange node IDs.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NodeId {
    graph: u64,
    index: u16,
}

/// Editable continuous rectangle geometry; coverage is computed only during evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VectorRectangle {
    pub(crate) bounds: [f64; 4],
    pub(crate) color: Pixel,
}
impl VectorRectangle {
    /// Create document-space edges `[left, top, right, bottom]` and linear fill color.
    /// Geometry may extend outside the raster evaluation extent, including negative coordinates.
    /// # Errors
    /// Rejects nonfinite coordinates and empty rectangles.
    pub fn new(bounds: [f64; 4], color: Pixel) -> Result<Self, ImageError> {
        if bounds.iter().any(|v| !v.is_finite()) || bounds[0] >= bounds[2] || bounds[1] >= bounds[3]
        {
            return Err(ImageError::Invalid("Invalid vector rectangle"));
        }
        Ok(Self { bounds, color })
    }
    /// Original editable document-space geometry.
    pub const fn bounds(self) -> [f64; 4] {
        self.bounds
    }
    /// Original linear premultiplied fill color.
    pub const fn color(self) -> Pixel {
        self.color
    }
}

/// Image semantics independent of evaluation backend, storage and presentation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Operation {
    /// Read immutable canonical pixels from a bounded source adapter.
    Raster {
        /// Stable content identity.
        asset: AssetId,
        /// Source pixel dimensions.
        extent: Extent,
    },
    /// Uniform image without proportional source storage.
    Solid {
        /// Evaluation dimensions.
        extent: Extent,
        /// Constant linear premultiplied value.
        color: Pixel,
    },
    /// Analytically covered vector rectangle retaining original geometry.
    Rectangle {
        /// Clipping/evaluation dimensions.
        extent: Extent,
        /// Editable geometry and fill.
        shape: VectorRectangle,
    },
    /// Lanczos-3 pixel-center resampling with clamped source edges and normalized weights.
    /// A step reduces either axis by at most two; larger reductions use explicit pyramid nodes.
    Resample {
        /// Earlier graph node.
        input: NodeId,
        /// Output dimensions.
        extent: Extent,
        /// Explicit global sampling phase and inverse scale.
        sampling: Sampling,
    },
    /// Multiply linear RGB by `2^stops`, preserving alpha.
    Exposure {
        /// Earlier graph node.
        input: NodeId,
        /// Finite exposure in EV, between -32 and 32.
        stops: f32,
    },
    /// Composite equal-sized images with Porter-Duff source-over in linear light.
    Over {
        /// Background image.
        background: NodeId,
        /// Foreground image.
        foreground: NodeId,
    },
    /// Multiply all premultiplied input channels by the mask's alpha coverage.
    Mask {
        /// Image being masked.
        input: NodeId,
        /// Equal-sized image supplying alpha coverage.
        mask: NodeId,
    },
    /// Interpolate an adjustment against its original composite using authored mask coverage.
    /// Unlike source-over, this preserves unchanged alpha when an adjustment preserves alpha.
    Mix {
        /// Original composite.
        original: NodeId,
        /// Adjusted composite.
        adjusted: NodeId,
        /// Alpha coverage controlling the adjustment.
        mask: NodeId,
        /// Finite adjustment strength in `[0, 1]`.
        opacity: f32,
    },
}
impl Operation {
    pub(crate) fn inputs(self) -> [Option<NodeId>; 3] {
        match self {
            Self::Raster { .. } | Self::Solid { .. } | Self::Rectangle { .. } => [None; 3],
            Self::Resample { input, .. } | Self::Exposure { input, .. } => {
                [Some(input), None, None]
            }
            Self::Over { background, foreground } => [Some(background), Some(foreground), None],
            Self::Mask { input, mask } => [Some(input), Some(mask), None],
            Self::Mix { original, adjusted, mask, .. } => {
                [Some(original), Some(adjusted), Some(mask)]
            }
        }
    }
}

pub(crate) struct Entry {
    pub(crate) operation: Operation,
    pub(crate) extent: Extent,
    depth: u16,
    fingerprint: [u8; 32],
}

/// A bounded acyclic image graph. Appending requires existing inputs; nodes never mutate.
/// Document edits compile a new snapshot; structural fingerprints allow compatible reuse.
pub struct Graph {
    entries: Vec<Entry>,
    identity: u64,
    limit: usize,
    _memory: MemoryClaim,
}
impl Graph {
    /// Allocate a bounded node inventory from execution-admitted memory.
    /// # Errors
    /// Rejects zero/excessive capacity, exhausted identity, allocation or memory pressure.
    pub fn new(capacity: usize, memory: &MemoryLease) -> Result<Self, ImageError> {
        if capacity == 0 || capacity > MAX_NODES {
            return Err(ImageError::Invalid("Invalid graph capacity"));
        }
        let claim = memory.claim((capacity * size_of::<Entry>()) as u64)?;
        let mut entries = Vec::new();
        entries.try_reserve_exact(capacity)?;
        let identity = NEXT_GRAPH
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| ImageError::Invalid("Graph identity exhausted"))?;
        Ok(Self { entries, identity, limit: capacity, _memory: claim })
    }
    /// Append a validated operation; existing nodes remain immutable.
    /// # Errors
    /// Rejects foreign inputs, size mismatches, unsupported parameters and exhausted capacity/depth.
    pub fn add(&mut self, operation: Operation) -> Result<NodeId, ImageError> {
        if self.entries.len() == self.limit {
            return Err(ImageError::Invalid("Graph node capacity exhausted"));
        }
        let mut depth = 1;
        for input in operation.inputs().into_iter().flatten() {
            depth = depth.max(self.entry(input)?.depth + 1);
        }
        if depth > MAX_DEPTH {
            return Err(ImageError::Invalid("Image graph depth exceeded"));
        }
        let extent = match operation {
            Operation::Raster { extent, .. }
            | Operation::Solid { extent, .. }
            | Operation::Rectangle { extent, .. } => extent,
            Operation::Resample { input, extent, sampling } => {
                let source = self.extent(input)?;
                sampling.validate(source, extent)?;
                extent
            }
            Operation::Exposure { input, stops } => {
                if !stops.is_finite() || !(-32.0..=32.0).contains(&stops) {
                    return Err(ImageError::Invalid("Invalid exposure"));
                }
                self.extent(input)?
            }
            Operation::Over { background, foreground } => {
                self.equal_extent(background, foreground)?
            }
            Operation::Mask { input, mask } => self.equal_extent(input, mask)?,
            Operation::Mix { original, adjusted, mask, opacity } => {
                if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
                    return Err(ImageError::Invalid("Invalid adjustment strength"));
                }
                self.equal_extent(original, adjusted)?;
                self.equal_extent(original, mask)?
            }
        };
        let fingerprint = self.hash(operation, extent)?;
        let id = NodeId { graph: self.identity, index: self.entries.len() as u16 };
        self.entries.push(Entry { operation, extent, depth, fingerprint });
        Ok(id)
    }
    /// Resize a complete image with explicit aligned-corner Lanczos semantics.
    /// # Errors
    /// Rejects foreign nodes, excessive reduction, graph capacity/depth and invalid parameters.
    pub fn resize(&mut self, input: NodeId, extent: Extent) -> Result<NodeId, ImageError> {
        let sampling = Sampling::resize(self.extent(input)?, extent);
        self.add(Operation::Resample { input, extent, sampling })
    }
    /// Append explicit Lanczos pyramid nodes through a 1x1 image, retaining the source as level zero.
    /// Odd dimensions round upward independently; the complete required node capacity is checked first.
    /// # Errors
    /// Rejects a foreign input, insufficient capacity or excessive graph depth without partial changes.
    pub fn pyramid(&mut self, input: NodeId) -> Result<Pyramid, ImageError> {
        let entry = self.entry(input)?;
        let mut extent = entry.extent;
        let mut extra = 0;
        while extent.width() > 1 || extent.height() > 1 {
            extra += 1;
            extent = Extent::new(extent.width().div_ceil(2), extent.height().div_ceil(2))?;
        }
        if self.entries.len() + extra > self.limit
            || usize::from(entry.depth) + extra > usize::from(MAX_DEPTH)
        {
            return Err(ImageError::Invalid("Pyramid exceeds graph capacity or depth"));
        }
        let mut levels = [input; 32];
        let mut extent = self.extent(input)?;
        for level in 1..=extra {
            extent = Extent::new(extent.width().div_ceil(2), extent.height().div_ceil(2))?;
            levels[level] = self.resize(levels[level - 1], extent)?;
        }
        Ok(Pyramid { levels, length: extra + 1 })
    }
    /// Output dimensions for a node in this graph.
    /// # Errors
    /// Rejects foreign or unavailable node identities.
    pub fn extent(&self, node: NodeId) -> Result<Extent, ImageError> {
        Ok(self.entry(node)?.extent)
    }
    /// Borrow the immutable operation for CPU/GPU planning.
    /// # Errors
    /// Rejects foreign or unavailable node identities.
    pub fn operation(&self, node: NodeId) -> Result<Operation, ImageError> {
        Ok(self.entry(node)?.operation)
    }
    /// Structural content key including operation version, parameters, dimensions and input keys.
    /// This is independent of process-local IDs; source adapters must bind assets to exact content.
    /// # Errors
    /// Rejects foreign or unavailable nodes.
    pub fn fingerprint(&self, node: NodeId) -> Result<[u8; 32], ImageError> {
        Ok(self.entry(node)?.fingerprint)
    }
    pub(crate) fn entry(&self, node: NodeId) -> Result<&Entry, ImageError> {
        if node.graph != self.identity {
            return Err(ImageError::Invalid("Foreign graph node"));
        }
        self.entries.get(usize::from(node.index)).ok_or(ImageError::Invalid("Unknown graph node"))
    }
    fn equal_extent(&self, a: NodeId, b: NodeId) -> Result<Extent, ImageError> {
        let extent = self.extent(a)?;
        if extent != self.extent(b)? {
            return Err(ImageError::Invalid("Image inputs have different extents"));
        }
        Ok(extent)
    }
    fn hash(&self, operation: Operation, extent: Extent) -> Result<[u8; 32], ImageError> {
        let mut hash = Sha256::new();
        hash.update(b"sugarsubstitute-image-ops-linear-srgb-lanczos3-clamp-v1\0");
        hash.update(extent.width().to_le_bytes());
        hash.update(extent.height().to_le_bytes());
        for input in operation.inputs().into_iter().flatten() {
            hash.update(self.fingerprint(input)?);
        }
        let color = match operation {
            Operation::Raster { asset, .. } => {
                hash.update([0]);
                hash.update(asset.0);
                None
            }
            Operation::Solid { color, .. } => {
                hash.update([1]);
                Some(color)
            }
            Operation::Rectangle { shape, .. } => {
                hash.update([2]);
                for coordinate in shape.bounds {
                    hash.update(coordinate.to_le_bytes());
                }
                Some(shape.color)
            }
            Operation::Resample { sampling, .. } => {
                hash.update([3]);
                for value in sampling.scale().into_iter().chain(sampling.offset()) {
                    hash.update(value.to_le_bytes());
                }
                None
            }
            Operation::Exposure { stops, .. } => {
                hash.update([4]);
                hash.update(stops.to_le_bytes());
                None
            }
            Operation::Over { .. } => {
                hash.update([5]);
                None
            }
            Operation::Mask { .. } => {
                hash.update([6]);
                None
            }
            Operation::Mix { opacity, .. } => {
                hash.update([7]);
                hash.update(opacity.to_le_bytes());
                None
            }
        };
        if let Some(color) = color {
            for channel in color.channels() {
                hash.update(channel.to_le_bytes());
            }
        }
        Ok(hash.finalize().into())
    }
}

/// Explicit pyramid node inventory; no separate raster-processing pipeline or proportional storage.
pub struct Pyramid {
    levels: [NodeId; 32],
    length: usize,
}
impl Pyramid {
    /// Levels from the original image to 1x1, each axis rounded upward on reduction.
    pub fn levels(&self) -> &[NodeId] {
        &self.levels[..self.length]
    }
}
