// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Change a coverage's extent: place it inside a larger or smaller canvas, or resample it.
//!
//! Both operations build the result tile by tile, so their cost follows the target extent and
//! uncovered target tiles store nothing. Scratch buffers are charged to the running job; the
//! result's tiles are charged to the lease that will retain them.
use super::{Coverage, CoverageTile, FULL_COVERAGE, Slot, TILE_VALUES, Tiles, tile_index};
use crate::{Extent, ImageError, Region, TILE_EDGE, check};
use std::{collections::BTreeMap, sync::Arc};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryLease};

/// How resampled coverage is reconstructed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageResampling {
    /// Every target pixel takes the source pixel under its center, keeping hard edges.
    Nearest,
    /// A triangle filter widened by the reduction factor: bilinear when enlarging and
    /// area-weighted when reducing, so no source pixel is skipped.
    Smooth,
}

/// The source taps of one target row or column of a smooth resample.
struct Taps {
    start: u32,
    weights: Vec<f32>,
}

impl Coverage {
    /// Place this coverage inside a new `extent`, its origin at `offset` in target pixels.
    /// Target pixels outside the placed coverage are uncovered and source pixels outside the
    /// target are cropped, as a canvas resize does. `memory` funds the result's tiles.
    /// # Errors
    /// Preserves cancellation, deadline and memory pressure.
    pub fn reframe(
        &self,
        extent: Extent,
        offset: [i64; 2],
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<Self, ImageError> {
        let source = self.extent;
        Self::assemble(extent, memory, context, |part, values| {
            values.fill(0);
            let left = i64::from(part.x()) - offset[0];
            let top = i64::from(part.y()) - offset[1];
            let right = (left + i64::from(part.width())).min(i64::from(source.width()));
            let bottom = (top + i64::from(part.height())).min(i64::from(source.height()));
            let (read_left, read_top) = (left.max(0), top.max(0));
            if read_left >= right || read_top >= bottom {
                return Ok(());
            }
            let read = Region::new(
                read_left as u32,
                read_top as u32,
                (right - read_left) as u32,
                (bottom - read_top) as u32,
            )?;
            let (mut copied, _claim) = scratch(read.pixels(), 0_u16, context)?;
            self.read(read, &mut copied)?;
            let width = read.width() as usize;
            let column = (read_left - left) as usize;
            for (row, line) in copied.chunks_exact(width).enumerate() {
                let start = (row + (read_top - top) as usize) * part.width() as usize + column;
                values[start..start + width].copy_from_slice(line);
            }
            Ok(())
        })
    }

    /// Resample this coverage to `extent`, mapping pixel centers proportionally on each axis.
    /// `memory` funds the result's tiles.
    /// # Errors
    /// Preserves cancellation, deadline and memory pressure.
    pub fn resample(
        &self,
        extent: Extent,
        mode: CoverageResampling,
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<Self, ImageError> {
        if self.uniform() == Some(0) {
            return Self::new(extent, 0, memory);
        }
        match mode {
            CoverageResampling::Nearest => {
                let (columns, _columns) = nearest(self.extent.width(), extent.width(), context)?;
                let (rows, _rows) = nearest(self.extent.height(), extent.height(), context)?;
                Self::assemble(extent, memory, context, |part, values| {
                    let width = part.width() as usize;
                    for (row, line) in values.chunks_exact_mut(width).enumerate() {
                        let y = rows[part.y() as usize + row];
                        for (column, value) in line.iter_mut().enumerate() {
                            *value = self.value(columns[part.x() as usize + column], y);
                        }
                    }
                    Ok(())
                })
            }
            CoverageResampling::Smooth => {
                let columns = taps(self.extent.width(), extent.width(), context)?;
                let rows = taps(self.extent.height(), extent.height(), context)?;
                Self::assemble(extent, memory, context, |part, values| {
                    self.smooth_part(part, &columns, &rows, values, context)
                })
            }
        }
    }

    /// Filter one target tile from the source rows and columns its taps reach.
    fn smooth_part(
        &self,
        part: Region,
        columns: &[Taps],
        rows: &[Taps],
        values: &mut [u16],
        context: &JobContext,
    ) -> Result<(), ImageError> {
        let span = |taps: &[Taps]| {
            let start = taps.iter().map(|tap| tap.start).min().unwrap_or(0);
            let end = taps.iter().map(|tap| tap.start + tap.weights.len() as u32).max();
            (start, end.unwrap_or(start))
        };
        let part_columns = &columns[part.x() as usize..part.right() as usize];
        let part_rows = &rows[part.y() as usize..part.bottom() as usize];
        let (left, right) = span(part_columns);
        let (top, bottom) = span(part_rows);
        let read = Region::new(left, top, right - left, bottom - top)?;
        let (mut source, _source) = scratch(read.pixels(), 0_u16, context)?;
        self.read(read, &mut source)?;
        let (read_width, width) = (read.width() as usize, part.width() as usize);
        // The horizontal pass keeps every source row the part's vertical taps reach.
        let (mut horizontal, _horizontal) =
            scratch(u64::from(read.height()) * width as u64, 0.0_f32, context)?;
        for (line, filtered) in
            source.chunks_exact(read_width).zip(horizontal.chunks_exact_mut(width))
        {
            for (tap, output) in part_columns.iter().zip(filtered.iter_mut()) {
                let first = (tap.start - left) as usize;
                *output = tap
                    .weights
                    .iter()
                    .zip(&line[first..first + tap.weights.len()])
                    .map(|(weight, value)| weight * f32::from(*value))
                    .sum();
            }
        }
        check(context)?;
        for (tap, line) in part_rows.iter().zip(values.chunks_exact_mut(width)) {
            let first = (tap.start - top) as usize;
            for (column, value) in line.iter_mut().enumerate() {
                let sum: f32 = tap
                    .weights
                    .iter()
                    .enumerate()
                    .map(|(offset, weight)| weight * horizontal[(first + offset) * width + column])
                    .sum();
                *value = sum.round().clamp(0.0, f32::from(FULL_COVERAGE)) as u16;
            }
        }
        Ok(())
    }

    /// Build coverage over `extent` one tile at a time; `fill` writes a tile's row-major values.
    fn assemble(
        extent: Extent,
        memory: &MemoryLease,
        context: &JobContext,
        mut fill: impl FnMut(Region, &mut [u16]) -> Result<(), ImageError>,
    ) -> Result<Self, ImageError> {
        let mut slots = BTreeMap::new();
        let columns = extent.width().div_ceil(TILE_EDGE);
        let (mut part_values, _claim) = scratch(u64::from(TILE_EDGE * TILE_EDGE), 0_u16, context)?;
        for part in extent.region().tiles() {
            check(context)?;
            let values = &mut part_values[..part.pixels() as usize];
            fill(part, values)?;
            let key = u64::from(part.y() / TILE_EDGE) * u64::from(columns)
                + u64::from(part.x() / TILE_EDGE);
            let first = values[0];
            if values.iter().all(|value| *value == first) {
                if first != 0 {
                    slots.insert(key, Slot::Uniform(first));
                }
                continue;
            }
            let mut tile = Vec::new();
            tile.try_reserve_exact(TILE_VALUES)?;
            tile.resize(TILE_VALUES, 0);
            for (row, line) in values.chunks_exact(part.width() as usize).enumerate() {
                let start = tile_index(part.x(), part.y() + row as u32);
                tile[start..start + line.len()].copy_from_slice(line);
            }
            slots.insert(key, Slot::Stored(Arc::new(CoverageTile::new(tile, memory)?)));
        }
        Ok(Self { extent, columns, tiles: Arc::new(Tiles::new(extent, 0, slots, memory)?) })
    }
}

/// A scratch buffer of `length` values and the job claim that funds it.
fn scratch<T: Clone>(
    length: u64,
    value: T,
    context: &JobContext,
) -> Result<(Vec<T>, MemoryClaim), ImageError> {
    let bytes = length
        .checked_mul(size_of::<T>() as u64)
        .ok_or(ImageError::Invalid("Coverage scratch overflow"))?;
    let claim = context.memory().claim(bytes + 64)?;
    let length =
        usize::try_from(length).map_err(|_| ImageError::Invalid("Coverage scratch overflow"))?;
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(length)?;
    buffer.resize(length, value);
    Ok((buffer, claim))
}

/// The source index under each target pixel center along one axis.
fn nearest(
    source: u32,
    target: u32,
    context: &JobContext,
) -> Result<(Vec<u32>, MemoryClaim), ImageError> {
    let (mut indices, claim) = scratch(u64::from(target), 0_u32, context)?;
    let ratio = f64::from(source) / f64::from(target);
    for (index, value) in indices.iter_mut().enumerate() {
        *value = (((index as f64 + 0.5) * ratio).floor() as u32).min(source - 1);
    }
    Ok((indices, claim))
}

/// Normalized triangle-filter taps for each target position along one axis.
fn taps(source: u32, target: u32, context: &JobContext) -> Result<Vec<Taps>, ImageError> {
    let ratio = f64::from(source) / f64::from(target);
    let support = ratio.max(1.0);
    let mut all = Vec::new();
    all.try_reserve_exact(target as usize)?;
    for index in 0..target {
        if index % TILE_EDGE == 0 {
            check(context)?;
        }
        let center = (f64::from(index) + 0.5) * ratio;
        let start = (center - support).floor().max(0.0) as u32;
        let end = ((center + support).ceil() as u32).min(source);
        let mut weights: Vec<f32> = (start..end)
            .map(|tap| (1.0 - ((f64::from(tap) + 0.5 - center) / support).abs()).max(0.0) as f32)
            .collect();
        let total: f32 = weights.iter().sum();
        if total > 0.0 {
            weights.iter_mut().for_each(|weight| *weight /= total);
        } else {
            // A target center beyond every source center takes the nearest edge pixel.
            let edge = (center.floor() as u32).min(source - 1);
            all.push(Taps { start: edge, weights: vec![1.0] });
            continue;
        }
        all.push(Taps { start, weights });
    }
    Ok(all)
}
