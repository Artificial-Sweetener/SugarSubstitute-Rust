// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pixel-edge selection boundaries, as CuteCanvas's `SelectionBoundaryBuilder` traces them.
//!
//! A pixel is selected when its 8-bit coverage is at least 128. Every pixel edge separating a
//! selected pixel from an unselected one, or from outside the coverage extent, is part of the
//! boundary. Edges are merged into maximal straight runs: horizontal runs row by row, left to
//! right, then vertical runs column by column, top to bottom. Each run starts its own dash
//! pattern at its top or left end, as each run is its own subpath in CuteCanvas.
use crate::CanvasError;
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::{Coverage, Extent, ImageError, Region, coverage_to_u8};

/// Lowest 8-bit coverage of a selected pixel.
const SELECTED: u8 = 128;
/// Runs admitted per memory claim while tracing.
const CHUNK: usize = 4096;

/// One straight run of pixel edges in document pixel coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundaryRun {
    /// Top or left end of the run, on the pixel-corner grid.
    pub start: [u32; 2],
    /// Length in document pixels.
    pub length: u32,
    /// Whether the run goes down from `start` rather than right.
    pub vertical: bool,
}
impl BoundaryRun {
    /// Bottom or right end of the run.
    pub const fn end(self) -> [u32; 2] {
        let [x, y] = self.start;
        if self.vertical { [x, y + self.length] } else { [x + self.length, y] }
    }
}

/// Every boundary run of one coverage, identified by that coverage's content fingerprint.
#[derive(Debug)]
pub struct CoverageBoundary {
    fingerprint: [u8; 32],
    extent: Extent,
    runs: Vec<BoundaryRun>,
    _memory: Vec<MemoryClaim>,
}
impl CoverageBoundary {
    /// Trace a coverage's boundary inside an owned job; cost follows its covered bounds.
    /// # Errors
    /// Preserves cancellation, deadline and memory pressure.
    pub fn trace(coverage: &Coverage, context: &JobContext) -> Result<Self, CanvasError> {
        let mut tracer = Tracer { runs: Vec::new(), memory: Vec::new(), context };
        if let Some(bounds) = coverage.bounds() {
            tracer.trace(coverage, bounds)?;
        }
        Ok(Self {
            fingerprint: coverage.fingerprint(),
            extent: coverage.extent(),
            runs: tracer.runs,
            _memory: tracer.memory,
        })
    }
    /// Content fingerprint of the traced coverage.
    pub const fn fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }
    /// Extent of the traced coverage.
    pub const fn extent(&self) -> Extent {
        self.extent
    }
    /// Whether no pixel is selected.
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }
    /// Horizontal runs in row order, then vertical runs in column order.
    pub fn runs(&self) -> &[BoundaryRun] {
        &self.runs
    }
}

struct Tracer<'a> {
    runs: Vec<BoundaryRun>,
    memory: Vec<MemoryClaim>,
    context: &'a JobContext,
}
impl Tracer<'_> {
    fn trace(&mut self, coverage: &Coverage, bounds: Region) -> Result<(), CanvasError> {
        let width = bounds.width() as usize;
        let bytes =
            (width * (size_of::<u16>() + 2) + (width + 1) * size_of::<Option<u32>>()) as u64;
        let _rows = self.context.memory().claim(bytes + 64).map_err(ImageError::from)?;
        let mut values = filled(width, 0_u16)?;
        let mut above = filled(width, false)?;
        let mut below = filled(width, false)?;
        let mut open: Vec<Option<u32>> = filled(width + 1, None)?;
        let mut vertical = Vec::new();
        for row in 0..=bounds.height() {
            check(self.context)?;
            if row < bounds.height() {
                coverage.read(
                    Region::new(bounds.x(), bounds.y() + row, bounds.width(), 1)?,
                    &mut values,
                )?;
                for (selected, value) in below.iter_mut().zip(&values) {
                    *selected = coverage_to_u8(*value) >= SELECTED;
                }
            } else {
                below.fill(false);
            }
            let mut start = None;
            for x in 0..=width {
                let edge = x < width && above[x] != below[x];
                match (edge, start) {
                    (true, None) => start = Some(x),
                    (false, Some(first)) => {
                        let run = BoundaryRun {
                            start: [bounds.x() + first as u32, bounds.y() + row],
                            length: (x - first) as u32,
                            vertical: false,
                        };
                        self.push(run, None)?;
                        start = None;
                    }
                    _ => {}
                }
            }
            for (x, open) in open.iter_mut().enumerate() {
                let left = x > 0 && below[x - 1];
                let right = x < width && below[x];
                match (left != right && row < bounds.height(), *open) {
                    (true, None) => *open = Some(row),
                    (false, Some(first)) => {
                        let run = BoundaryRun {
                            start: [bounds.x() + x as u32, bounds.y() + first],
                            length: row - first,
                            vertical: true,
                        };
                        self.push(run, Some(&mut vertical))?;
                        *open = None;
                    }
                    _ => {}
                }
            }
            std::mem::swap(&mut above, &mut below);
        }
        vertical.sort_unstable_by_key(|run: &BoundaryRun| (run.start[0], run.start[1]));
        self.runs.try_reserve_exact(vertical.len()).map_err(ImageError::from)?;
        self.runs.extend(vertical);
        Ok(())
    }
    /// Append a run to the boundary or to `staging`, admitting storage in chunks. Staged runs
    /// are charged as if they were already in the boundary, which they join at the end.
    fn push(
        &mut self,
        run: BoundaryRun,
        staging: Option<&mut Vec<BoundaryRun>>,
    ) -> Result<(), CanvasError> {
        let target = match staging {
            Some(staging) => staging,
            None => &mut self.runs,
        };
        if target.len() == target.capacity() {
            let bytes = (2 * CHUNK * size_of::<BoundaryRun>()) as u64;
            self.memory.push(self.context.memory().claim(bytes).map_err(ImageError::from)?);
            target.try_reserve_exact(CHUNK).map_err(ImageError::from)?;
        }
        target.push(run);
        Ok(())
    }
}

fn filled<T: Clone>(length: usize, value: T) -> Result<Vec<T>, CanvasError> {
    let mut values = Vec::new();
    values.try_reserve_exact(length).map_err(ImageError::from)?;
    values.resize(length, value);
    Ok(values)
}

fn check(context: &JobContext) -> Result<(), CanvasError> {
    if context.cancellation_requested() {
        return Err(ImageError::Cancelled.into());
    }
    if context.deadline().is_some_and(|deadline| context.now() >= deadline) {
        return Err(ImageError::Deadline.into());
    }
    Ok(())
}
