// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Spatial contracts shared by evaluation, dirty-region propagation and backend planning.
use crate::{Extent, Graph, ImageError, NodeId, Operation, Region};

impl Graph {
    /// Required input regions for an exact output region, including Lanczos filter halos.
    /// Source edges clamp during resampling; other current operations preserve pixel coordinates.
    /// # Errors
    /// Rejects foreign nodes and output regions outside their evaluation extent.
    pub fn required_inputs(
        &self,
        node: NodeId,
        output: Region,
    ) -> Result<[Option<(NodeId, Region)>; 2], ImageError> {
        let entry = self.entry(node)?;
        if output.intersection(entry.extent.region()) != Some(output) {
            return Err(ImageError::Invalid("Requested region is outside node extent"));
        }
        let inputs = match entry.operation {
            Operation::Resample { input, extent } => {
                let source = self.extent(input)?;
                let (left, right) =
                    required_axis(output.x(), output.width(), source.width(), extent.width());
                let (top, bottom) =
                    required_axis(output.y(), output.height(), source.height(), extent.height());
                [Some((input, Region::new(left, top, right - left, bottom - top)?)), None]
            }
            operation => operation.inputs().map(|id| id.map(|id| (id, output))),
        };
        Ok(inputs)
    }
    /// Conservatively map a changed direct input region into this node's dirty output region.
    /// The caller propagates this mapping through dependent nodes; unrelated branches remain valid.
    /// # Errors
    /// Rejects foreign nodes and an input that is not a direct dependency.
    pub fn invalidated_by(
        &self,
        node: NodeId,
        input: NodeId,
        changed: Region,
    ) -> Result<Option<Region>, ImageError> {
        let entry = self.entry(node)?;
        if !entry.operation.inputs().contains(&Some(input)) {
            return Err(ImageError::Invalid("Node does not depend on changed input"));
        }
        let source = self.extent(input)?;
        let Some(changed) = changed.intersection(source.region()) else {
            return Ok(None);
        };
        match entry.operation {
            Operation::Resample { extent, .. } => {
                let (left, right) =
                    invalidated_axis(changed.x(), changed.right(), source.width(), extent.width());
                let (top, bottom) = invalidated_axis(
                    changed.y(),
                    changed.bottom(),
                    source.height(),
                    extent.height(),
                );
                if left == right || top == bottom {
                    return Ok(None);
                }
                Ok(Some(Region::new(left, top, right - left, bottom - top)?))
            }
            _ => Ok(changed.intersection(entry.extent.region())),
        }
    }
}

/// Include every nonzero tap for all output pixel centers, rounded outward before edge clipping.
fn required_axis(start: u32, length: u32, source: u32, target: u32) -> (u32, u32) {
    let ratio = f64::from(source) / f64::from(target);
    let radius = 3.0 * ratio.max(1.0);
    let first = (f64::from(start) + 0.5) * ratio - 0.5;
    let last = (f64::from(start + length - 1) + 0.5) * ratio - 0.5;
    let left = (first - radius).floor().clamp(0.0, f64::from(source - 1)) as u32;
    let right = (last + radius + 1.0).ceil().clamp(f64::from(left + 1), f64::from(source)) as u32;
    (left, right)
}

/// Dirty bounds include the complete support of each possibly influenced output sample.
fn invalidated_axis(start: u32, end: u32, source: u32, target: u32) -> (u32, u32) {
    let ratio = f64::from(source) / f64::from(target);
    let radius = 3.0 * ratio.max(1.0);
    let left = ((f64::from(start) - radius + 0.5) / ratio - 0.5)
        .floor()
        .clamp(0.0, f64::from(target)) as u32;
    let right = ((f64::from(end) + radius + 0.5) / ratio + 0.5)
        .ceil()
        .clamp(f64::from(left), f64::from(target)) as u32;
    (left, right)
}

pub(crate) fn tile_region(extent: Extent, x: u32, y: u32) -> Result<Region, ImageError> {
    let left =
        x.checked_mul(crate::TILE_EDGE).ok_or(ImageError::Invalid("Tile coordinate overflow"))?;
    let top =
        y.checked_mul(crate::TILE_EDGE).ok_or(ImageError::Invalid("Tile coordinate overflow"))?;
    if left >= extent.width() || top >= extent.height() {
        return Err(ImageError::Invalid("Tile outside image"));
    }
    Region::new(
        left,
        top,
        crate::TILE_EDGE.min(extent.width() - left),
        crate::TILE_EDGE.min(extent.height() - top),
    )
}
