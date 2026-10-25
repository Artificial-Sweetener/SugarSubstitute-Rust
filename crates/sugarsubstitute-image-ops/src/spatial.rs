// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Spatial contracts shared by evaluation, dirty-region propagation and backend planning.
use crate::{Filter, Graph, ImageError, NodeId, Operation, Region};

impl Graph {
    /// Required input regions for an exact output region, including Lanczos filter halos.
    /// Source edges clamp during resampling; other current operations preserve pixel coordinates.
    /// # Errors
    /// Rejects foreign nodes and output regions outside their evaluation extent.
    pub fn required_inputs(
        &self,
        node: NodeId,
        output: Region,
    ) -> Result<[Option<(NodeId, Region)>; 3], ImageError> {
        let entry = self.entry(node)?;
        if output.intersection(entry.extent.region()) != Some(output) {
            return Err(ImageError::Invalid("Requested region is outside node extent"));
        }
        let inputs = match entry.operation {
            Operation::Resample { input, sampling, .. } => {
                let source = self.extent(input)?;
                let (left, right) = required_axis(
                    output.x(),
                    output.width(),
                    source.width(),
                    sampling.scale()[0],
                    sampling.offset()[0],
                    sampling.filter(),
                    sampling.output_translation()[0],
                );
                let (top, bottom) = required_axis(
                    output.y(),
                    output.height(),
                    source.height(),
                    sampling.scale()[1],
                    sampling.offset()[1],
                    sampling.filter(),
                    sampling.output_translation()[1],
                );
                [Some((input, Region::new(left, top, right - left, bottom - top)?)), None, None]
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
            Operation::Resample { extent, sampling, .. } => {
                let (left, right) = invalidated_axis(
                    [changed.x(), changed.right()],
                    source.width(),
                    extent.width(),
                    sampling.scale()[0],
                    sampling.offset()[0],
                    sampling.filter(),
                    sampling.output_translation()[0],
                );
                let (top, bottom) = invalidated_axis(
                    [changed.y(), changed.bottom()],
                    source.height(),
                    extent.height(),
                    sampling.scale()[1],
                    sampling.offset()[1],
                    sampling.filter(),
                    sampling.output_translation()[1],
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
fn required_axis(
    start: u32,
    length: u32,
    source: u32,
    ratio: f64,
    offset: f64,
    filter: Filter,
    translation: i32,
) -> (u32, u32) {
    if source == 1 {
        return (0, 1);
    }
    let radius = 3.0 * ratio.max(1.0);
    let start = i64::from(start) - i64::from(translation);
    let first_center = crate::kernel::pixel_center(start, ratio, offset);
    let last_center = crate::kernel::pixel_center(start + i64::from(length) - 1, ratio, offset);
    if filter == Filter::Nearest {
        let left = first_center.floor().clamp(0.0, f64::from(source - 1)) as u32;
        let right = last_center.floor().clamp(f64::from(left), f64::from(source - 1)) as u32 + 1;
        return (left, right);
    }
    let first = first_center - 0.5;
    let last = last_center - 0.5;
    if ratio == 1.0 && offset.fract() == 0.0 {
        let left = first.clamp(0.0, f64::from(source - 1)) as u32;
        let right = (last + 1.0).clamp(f64::from(left + 1), f64::from(source)) as u32;
        return (left, right);
    }
    let left = (first - radius).floor().clamp(0.0, f64::from(source - 1)) as u32;
    let right = (last + radius + 1.0).ceil().clamp(f64::from(left + 1), f64::from(source)) as u32;
    (left, right)
}

/// Dirty bounds include the complete support of each possibly influenced output sample.
fn invalidated_axis(
    [start, end]: [u32; 2],
    source: u32,
    target: u32,
    ratio: f64,
    offset: f64,
    filter: Filter,
    translation: i32,
) -> (u32, u32) {
    let radius = match filter {
        Filter::Lanczos3 => 3.0 * ratio.max(1.0),
        Filter::Nearest => 0.5,
    };
    let left = if start == 0 {
        0
    } else {
        ((f64::from(start) - radius + 0.5 - offset) / ratio - 0.5 + f64::from(translation))
            .floor()
            .clamp(0.0, f64::from(target)) as u32
    };
    let right = if end == source {
        target
    } else {
        ((f64::from(end) + radius + 0.5 - offset) / ratio + 0.5 + f64::from(translation))
            .ceil()
            .clamp(f64::from(left), f64::from(target)) as u32
    };
    (left, right)
}
