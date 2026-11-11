// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The one set of coverage operations every coverage role uses.
//!
//! Combination is alpha compositing of coverage. Expand and contract are flat square
//! morphology, and feather approximates a Gaussian with three box passes; both treat
//! everything outside the extent as uncovered. Filters work on a dense copy of the affected
//! region only, so their cost follows the covered area rather than the document size. Coverage
//! that keeps content beyond its extent is filtered whole, as one layer raster.
use super::{Coverage, FULL_COVERAGE, beyond, multiply};
use crate::{ImageError, Region, check};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};

/// How incoming coverage joins existing coverage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CombineMode {
    /// Use the incoming coverage alone.
    Replace,
    /// Union: incoming over existing.
    Add,
    /// Remove incoming coverage from existing coverage.
    Subtract,
    /// Keep only coverage present in both.
    Intersect,
}

impl CombineMode {
    /// Combine one existing value with one incoming value.
    #[inline]
    pub const fn apply(self, existing: u16, incoming: u16) -> u16 {
        match self {
            Self::Replace => incoming,
            Self::Add => incoming + multiply(existing, FULL_COVERAGE - incoming),
            Self::Subtract => multiply(existing, FULL_COVERAGE - incoming),
            Self::Intersect => multiply(existing, incoming),
        }
    }
}

impl Coverage {
    /// Combine `incoming` into this coverage, content beyond the extent included.
    /// # Errors
    /// Rejects a different extent and preserves cancellation and memory pressure.
    pub fn combine(
        &self,
        incoming: &Self,
        mode: CombineMode,
        context: &JobContext,
    ) -> Result<Self, ImageError> {
        if incoming.extent != self.extent {
            return Err(ImageError::Invalid("Combined coverage extents differ"));
        }
        if mode == CombineMode::Replace {
            return Ok(incoming.clone());
        }
        if incoming.beyond.is_some() {
            let Some(frame) = beyond::union(self.content_bounds(), incoming.content_bounds())
            else {
                return Ok(self.clone());
            };
            let incoming = incoming.whole_in(frame, context.memory(), context)?;
            return self.edit_whole(frame, context.memory(), context, |whole| {
                whole.combine(&incoming, mode, context)
            });
        }
        if mode == CombineMode::Intersect && self.beyond.is_some() {
            // Nothing beyond the extent meets incoming coverage that lies wholly inside it.
            return self.without_outside().combine(incoming, mode, context);
        }
        // Only regions where either side is covered can change, apart from intersect, which
        // also clears everything outside the incoming coverage.
        let changed = match mode {
            CombineMode::Intersect => self.bounds(),
            _ => incoming.bounds(),
        };
        let Some(changed) = changed else {
            return Ok(self.clone());
        };
        let mut result = self.clone();
        result.update_in_job(changed, context, |x, y, existing| {
            mode.apply(existing, incoming.value(x, y))
        })?;
        Ok(result)
    }

    /// Grow coverage by a square pixel radius.
    /// # Errors
    /// Preserves cancellation and memory pressure.
    pub fn expand(&self, radius: u32, context: &JobContext) -> Result<Self, ImageError> {
        self.morphology(radius, true, context)
    }

    /// Shrink coverage by a square pixel radius; the extent's edges count as uncovered.
    /// # Errors
    /// Preserves cancellation and memory pressure.
    pub fn contract(&self, radius: u32, context: &JobContext) -> Result<Self, ImageError> {
        self.morphology(radius, false, context)
    }

    /// Soften edges by approximately a Gaussian of `radius` pixels: three box passes of radius
    /// `round(radius / √3)`, at least one.
    /// # Errors
    /// Rejects a negative or nonfinite radius and preserves cancellation and memory pressure.
    pub fn feather(&self, radius: f32, context: &JobContext) -> Result<Self, ImageError> {
        if !radius.is_finite() || radius < 0.0 {
            return Err(ImageError::Invalid("Invalid coverage feather radius"));
        }
        if radius == 0.0 {
            return Ok(self.clone());
        }
        let boxed = ((radius * 0.577_35).round() as u32).max(1);
        if self.beyond.is_some() {
            return self.grown_edit(3 * boxed, context, |whole| whole.feather(radius, context));
        }
        let Some(region) = self.affected(3 * boxed) else {
            return Ok(self.clone());
        };
        let (rounded, _claim) = {
            let (mut values, claims) = self.dense(region, context)?;
            let mut line = Vec::new();
            for _ in 0..3 {
                for vertical in [false, true] {
                    check(context)?;
                    box_pass(&mut values, region, boxed, vertical, &mut line)?;
                }
            }
            let mut rounded = Vec::new();
            rounded.try_reserve_exact(values.len())?;
            rounded.extend(values.iter().map(|value| value.round().clamp(0.0, 65535.0) as u16));
            drop((values, line));
            (rounded, claims.keep_values())
        };
        // The floating-point scratch is released before the result's tiles are charged, so the
        // job holds at most the dense copy's charge at once: a feathered result fits the same
        // reservation as an expanded or contracted one.
        let mut result = self.clone();
        result.write(region, &rounded, context.memory())?;
        Ok(result)
    }

    fn morphology(
        &self,
        radius: u32,
        grow: bool,
        context: &JobContext,
    ) -> Result<Self, ImageError> {
        if radius == 0 {
            return Ok(self.clone());
        }
        let margin = if grow { radius } else { 0 };
        if self.beyond.is_some() {
            return self
                .grown_edit(margin, context, |whole| whole.morphology(radius, grow, context));
        }
        let Some(region) = self.affected(margin) else {
            return Ok(self.clone());
        };
        let _claim = context.memory().claim(region.pixels() * 2 + 64)?;
        let mut values = Vec::new();
        values.try_reserve_exact(region.pixels() as usize)?;
        values.resize(region.pixels() as usize, 0);
        self.read(region, &mut values)?;
        let mut queue = Vec::new();
        for vertical in [false, true] {
            check(context)?;
            extreme_pass(&mut values, region, radius, grow, vertical, &mut queue)?;
        }
        let mut result = self.clone();
        result.write(region, &values, context.memory())?;
        Ok(result)
    }

    /// The covered bounds grown by `margin` and clipped to the extent.
    fn affected(&self, margin: u32) -> Option<Region> {
        let bounds = self.bounds()?;
        let left = bounds.x().saturating_sub(margin);
        let top = bounds.y().saturating_sub(margin);
        let right = bounds.right().saturating_add(margin).min(self.extent.width());
        let bottom = bounds.bottom().saturating_add(margin).min(self.extent.height());
        Region::new(left, top, right - left, bottom - top).ok()
    }

    /// Copy a region into floating-point working storage charged to the job.
    fn dense(
        &self,
        region: Region,
        context: &JobContext,
    ) -> Result<(Vec<f32>, DenseClaims), ImageError> {
        let count = region.pixels() as usize;
        let claims = DenseClaims {
            values: context.memory().claim(region.pixels() * 2 + 64)?,
            scratch: context.memory().claim(region.pixels() * 4 + 64)?,
        };
        let mut values = Vec::new();
        values.try_reserve_exact(count)?;
        values.resize(count, 0);
        self.read(region, &mut values)?;
        let mut dense = Vec::new();
        dense.try_reserve_exact(count)?;
        dense.extend(values.into_iter().map(f32::from));
        Ok((dense, claims))
    }
}

/// The job's charge for a dense working copy: two bytes per value for the integer values read
/// from the coverage and later written back, and four for the floating-point scratch.
struct DenseClaims {
    values: MemoryClaim,
    scratch: MemoryClaim,
}

impl DenseClaims {
    /// Release the scratch charge, keeping the one that funds the integer values.
    fn keep_values(self) -> MemoryClaim {
        drop(self.scratch);
        self.values
    }
}

/// One separable box-blur pass with zero padding outside the region, whose margin keeps every
/// nonzero value inside it.
fn box_pass(
    values: &mut [f32],
    region: Region,
    radius: u32,
    vertical: bool,
    line: &mut Vec<f32>,
) -> Result<(), ImageError> {
    let (length, lines) = axis(region, vertical);
    let width = region.width() as usize;
    let at = |line_index: usize, position: usize| {
        if vertical { position * width + line_index } else { line_index * width + position }
    };
    let window = (2 * radius + 1) as f32;
    let radius = radius as usize;
    line.clear();
    line.try_reserve_exact(length + 1)?;
    for line_index in 0..lines {
        line.clear();
        line.push(0.0);
        let mut sum = 0.0;
        for position in 0..length {
            sum += values[at(line_index, position)];
            line.push(sum);
        }
        for position in 0..length {
            let start = position.saturating_sub(radius);
            let end = (position + radius + 1).min(length);
            values[at(line_index, position)] = (line[end] - line[start]) / window;
        }
    }
    Ok(())
}

/// One separable flat max (grow) or min (shrink) pass over `2·radius + 1` samples, using a
/// monotonic queue so the cost is independent of the radius. Positions outside the region are
/// uncovered: the region holds every covered pixel, and the extent's edges count as uncovered.
fn extreme_pass(
    values: &mut [u16],
    region: Region,
    radius: u32,
    grow: bool,
    vertical: bool,
    queue: &mut Vec<(usize, u16)>,
) -> Result<(), ImageError> {
    let (length, lines) = axis(region, vertical);
    let width = region.width() as usize;
    let at = |line_index: usize, position: usize| {
        if vertical { position * width + line_index } else { line_index * width + position }
    };
    let radius = radius as usize;
    let mut output = Vec::new();
    output.try_reserve_exact(length)?;
    queue.clear();
    queue.try_reserve_exact(length + 2 * radius + 1)?;
    for line_index in 0..lines {
        output.clear();
        queue.clear();
        let mut head = 0;
        // Positions are offset by `radius` so the zero padding on the left is representable.
        let padded = length + 2 * radius;
        let sample = |position: usize| {
            if position < radius || position >= radius + length {
                0
            } else {
                values[at(line_index, position - radius)]
            }
        };
        let mut next = 0;
        for position in 0..length {
            let reach = position + 2 * radius;
            while next <= reach.min(padded - 1) {
                let value = sample(next);
                while queue.len() > head {
                    let last = queue[queue.len() - 1].1;
                    if if grow { last > value } else { last < value } {
                        break;
                    }
                    queue.pop();
                }
                queue.push((next, value));
                next += 1;
            }
            while queue[head].0 < position {
                head += 1;
            }
            output.push(queue[head].1);
        }
        for (position, value) in output.iter().enumerate() {
            values[at(line_index, position)] = *value;
        }
    }
    Ok(())
}

/// Line length and count for a pass along one axis.
fn axis(region: Region, vertical: bool) -> (usize, usize) {
    if vertical {
        (region.height() as usize, region.width() as usize)
    } else {
        (region.width() as usize, region.height() as usize)
    }
}
