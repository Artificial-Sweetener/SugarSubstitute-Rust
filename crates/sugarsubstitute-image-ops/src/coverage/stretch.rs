// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stretch coverage along its axes, as resizing a shared edge between masks does.
//!
//! One axis-aligned map carries a source rectangle onto a target rectangle and applies to the
//! whole coverage. Each target pixel averages the source over its exact inverse-mapped footprint,
//! so a mask whose edges land on whole pixels keeps hard, gap-free edges at any stretch, and an
//! identity map copies values exactly. Target rows are built in tile-high strips so scratch
//! memory follows one strip rather than the whole extent.
use super::Coverage;
use crate::{ImageError, Region, TILE_EDGE, check};
use sugarsubstitute_contracts::execution::JobContext;

/// `target = scale · source + offset` along one axis; `scale` is positive.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Axis {
    scale: f64,
    offset: f64,
}

/// The source pixels one target pixel averages, with their footprint weights.
struct Taps {
    start: u32,
    weights: Vec<f64>,
}

impl Axis {
    fn new(from: [f64; 2], to: [f64; 2]) -> Result<Self, ImageError> {
        let (source, target) = (from[1] - from[0], to[1] - to[0]);
        if ![from[0], from[1], to[0], to[1]].iter().all(|value| value.is_finite())
            || source <= 0.0
            || target <= 0.0
        {
            return Err(ImageError::Invalid("Stretched coverage needs nonempty finite spans"));
        }
        let scale = target / source;
        Ok(Self { scale, offset: to[0] - scale * from[0] })
    }

    fn identity(self) -> bool {
        self.scale == 1.0 && self.offset == 0.0
    }

    /// Target pixels `[start, end)` reached by source pixels `[low, high)`, inside `[0, limit)`.
    fn reach(self, low: u32, high: u32, limit: u32) -> Option<(u32, u32)> {
        let start = (self.scale * f64::from(low) + self.offset).floor().max(0.0);
        let end = (self.scale * f64::from(high) + self.offset).ceil().min(f64::from(limit));
        (start < end).then_some((start as u32, end as u32))
    }

    /// Footprint taps of target pixels `[start, end)` over source pixels `[low, high)`.
    fn taps(self, (start, end): (u32, u32), low: u32, high: u32) -> Result<Vec<Taps>, ImageError> {
        let mut all = Vec::new();
        all.try_reserve_exact((end - start) as usize)?;
        for target in start..end {
            let a = (f64::from(target) - self.offset) / self.scale;
            let b = (f64::from(target) + 1.0 - self.offset) / self.scale;
            let first = (a.floor().max(f64::from(low))) as u32;
            let last = (b.ceil().min(f64::from(high))).max(f64::from(first)) as u32;
            let weights = (first..last)
                .map(|pixel| {
                    let overlap = (f64::from(pixel) + 1.0).min(b) - f64::from(pixel).max(a);
                    overlap.max(0.0) / (b - a)
                })
                .collect();
            all.push(Taps { start: first, weights });
        }
        Ok(all)
    }
}

impl Coverage {
    /// This coverage under the axis-aligned map that carries `from` onto `to`, both
    /// `[left, top, right, bottom]` in document pixels; coverage leaving the extent is dropped.
    /// # Errors
    /// Rejects empty or nonfinite rectangles and preserves cancellation, deadline and memory
    /// pressure.
    pub fn stretch(
        &self,
        from: [f64; 4],
        to: [f64; 4],
        context: &JobContext,
    ) -> Result<Self, ImageError> {
        check(context)?;
        let columns = Axis::new([from[0], from[2]], [to[0], to[2]])?;
        let rows = Axis::new([from[1], from[3]], [to[1], to[3]])?;
        if columns.identity() && rows.identity() {
            return Ok(self.clone());
        }
        let mut stretched = Self::new(self.extent, 0, context.memory())?;
        let Some(source) = self.bounds() else {
            return Ok(stretched);
        };
        let (Some(across), Some(down)) = (
            columns.reach(source.x(), source.right(), self.extent.width()),
            rows.reach(source.y(), source.bottom(), self.extent.height()),
        ) else {
            return Ok(stretched);
        };
        let column_taps = columns.taps(across, source.x(), source.right())?;
        let row_taps = rows.taps(down, source.y(), source.bottom())?;
        let width = (across.1 - across.0) as usize;
        for (strip, taps) in row_taps.chunks(TILE_EDGE as usize).enumerate() {
            check(context)?;
            let top = down.0 + strip as u32 * TILE_EDGE;
            let reached = taps.iter().filter(|tap| !tap.weights.is_empty());
            let low = reached.clone().map(|tap| tap.start).min();
            let high = reached.map(|tap| tap.start + tap.weights.len() as u32).max();
            let mut values = scratch(width * taps.len(), 0_u16, context)?;
            if let (Some(low), Some(high)) = (low, high) {
                let read = Region::new(source.x(), low, source.width(), high - low)?;
                let mut lines = scratch(read.pixels() as usize, 0_u16, context)?;
                self.read(read, &mut lines.0)?;
                let mut horizontal = scratch(read.height() as usize * width, 0.0_f64, context)?;
                for (line, output) in lines
                    .0
                    .chunks_exact(read.width() as usize)
                    .zip(horizontal.0.chunks_exact_mut(width))
                {
                    for (tap, value) in column_taps.iter().zip(output.iter_mut()) {
                        let first = (tap.start - source.x()) as usize;
                        *value = tap
                            .weights
                            .iter()
                            .zip(&line[first..first + tap.weights.len()])
                            .map(|(weight, value)| weight * f64::from(*value))
                            .sum();
                    }
                }
                check(context)?;
                for (tap, output) in taps.iter().zip(values.0.chunks_exact_mut(width)) {
                    let first = tap.start.saturating_sub(low) as usize;
                    for (column, value) in output.iter_mut().enumerate() {
                        let sum: f64 = tap
                            .weights
                            .iter()
                            .enumerate()
                            .map(|(row, weight)| {
                                weight * horizontal.0[(first + row) * width + column]
                            })
                            .sum();
                        *value = sum.round().clamp(0.0, f64::from(u16::MAX)) as u16;
                    }
                }
            }
            let region = Region::new(across.0, top, width as u32, taps.len() as u32)?;
            stretched.write(region, &values.0, context.memory())?;
        }
        Ok(stretched)
    }
}

/// A scratch buffer of `length` values and the job claim that funds it.
fn scratch<T: Clone>(
    length: usize,
    value: T,
    context: &JobContext,
) -> Result<(Vec<T>, sugarsubstitute_contracts::execution::MemoryClaim), ImageError> {
    let claim = context.memory().claim((length * size_of::<T>()) as u64 + 64)?;
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(length)?;
    buffer.resize(length, value);
    Ok((buffer, claim))
}
