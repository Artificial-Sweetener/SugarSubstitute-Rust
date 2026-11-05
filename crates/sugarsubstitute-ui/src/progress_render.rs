// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Advance the progress owner's physical masks and gradient through bounded slices.

use super::{ProgressAppearance, ProgressBar, ProgressFrame, standalone_track};
use crate::accent::AccentPalette;
use crate::bitmap::{Bitmap, CoverageMask};
use crate::shapes::{AllocationError, CoverageRaster, PaintCursor, byte_product};
use tiny_skia::{PathBuilder, Rect};

enum Stage {
    Outer(CoverageRaster),
    Track,
    Fill,
    Paint(PaintCursor, bool),
    SweepMask(CoverageRaster),
    Merge(usize),
    Sweep(u32),
    Done,
}
/// Own at most the outer widget mask and one paint/sweep mask at any time.
pub struct ProgressCursor {
    stage: Stage,
    outer: Option<CoverageMask>,
    sweep: Option<CoverageMask>,
    bar: ProgressBar,
    frame: ProgressFrame,
    accent: AccentPalette,
    scale: f32,
    width: u32,
    height: u32,
    filled: f32,
    y: f32,
    bar_height: f32,
}
impl ProgressCursor {
    /// Capture the exact rounded child-widget clip without initializing its full plane.
    /// Completion is painted with the surface's accent palette.
    /// # Errors
    /// Rejects invalid geometry or completion and preserves allocation failures.
    pub fn new(
        width: u32,
        height: u32,
        bar: ProgressBar,
        frame: ProgressFrame,
        accent: AccentPalette,
        scale: f32,
    ) -> Result<Self, AllocationError> {
        if !bar.valid(scale)
            || !frame.is_valid()
            || width == 0
            || height == 0
            || width > 4096
            || height > 4096
        {
            return Err(AllocationError::InvalidDimensions);
        }
        let y = bar.y;
        let bar_height = bar.height;
        let left = (bar.x * scale).round();
        let top = (y * scale).round();
        let path = PathBuilder::from_rect(
            Rect::from_xywh(
                left,
                top,
                ((bar.x + bar.width) * scale).round() - left,
                ((y + bar_height) * scale).round() - top,
            )
            .ok_or(AllocationError::InvalidDimensions)?,
        );
        let stage = Stage::Outer(CoverageRaster::new(&path, width, height, true)?);
        let completed = bar.width * frame.completed;
        Ok(Self {
            stage,
            outer: None,
            sweep: None,
            bar,
            frame,
            accent,
            scale,
            width,
            height,
            filled: if matches!(bar.appearance, ProgressAppearance::Hairline { .. }) {
                completed.floor()
            } else {
                completed
            },
            y,
            bar_height,
        })
    }
    /// Initialize/integrate/merge <=32768 mask pixels or paint <=4 gradient columns.
    pub fn step(&mut self, target: &mut Bitmap) -> Result<bool, AllocationError> {
        if target.width() != self.width || target.height() != self.height {
            return Err(AllocationError::InvalidDimensions);
        }
        match &mut self.stage {
            Stage::Outer(raster) => {
                if !raster.step(32768) {
                    return Ok(false);
                }
                let Stage::Outer(raster) = std::mem::replace(&mut self.stage, Stage::Track) else {
                    return Err(AllocationError::InvalidDimensions);
                };
                self.outer = Some(raster.finish()?);
            }
            Stage::Track => {
                self.stage = Stage::Fill;
                let (_, track) = self.bar.colors(self.accent);
                if let ProgressAppearance::Hairline { dark } = self.bar.appearance {
                    standalone_track(target, self.bar, if dark { 255 } else { 0 }, self.scale);
                } else if let Some(path) = self.bar.clip(self.bar.width, self.scale) {
                    self.stage = Stage::Paint(
                        PaintCursor::new(&path, self.width, self.height, track, None, true)?,
                        true,
                    );
                }
            }
            Stage::Fill => {
                self.stage = Stage::Done;
                if let Some(path) = self.bar.clip(self.filled, self.scale) {
                    let (color, _) = self.bar.colors(self.accent);
                    self.stage = Stage::Paint(
                        PaintCursor::new(&path, self.width, self.height, color, None, true)?,
                        false,
                    );
                } else {
                    self.begin_sweep()?;
                }
            }
            Stage::Paint(cursor, track) => {
                if cursor.step(target, self.outer.as_ref(), 32768)? {
                    if *track {
                        self.stage = Stage::Fill;
                    } else {
                        self.begin_sweep()?;
                    }
                }
            }
            Stage::SweepMask(raster) => {
                if !raster.step(32768) {
                    return Ok(false);
                }
                let Stage::SweepMask(raster) = std::mem::replace(&mut self.stage, Stage::Merge(0))
                else {
                    return Err(AllocationError::InvalidDimensions);
                };
                self.sweep = Some(raster.finish()?);
            }
            Stage::Merge(offset) => {
                let sweep = self.sweep.as_mut().ok_or(AllocationError::InvalidDimensions)?;
                let outer = self.outer.as_ref().ok_or(AllocationError::InvalidDimensions)?;
                let end = (*offset + 32768).min(sweep.data().len());
                for (coverage, silhouette) in
                    sweep.data_mut()[*offset..end].iter_mut().zip(&outer.data()[*offset..end])
                {
                    *coverage = byte_product(*coverage, *silhouette);
                }
                *offset = end;
                if end == sweep.data().len() {
                    self.stage = Stage::Sweep((self.bar.x * self.scale).floor().max(0.0) as u32);
                }
            }
            Stage::Sweep(column) => {
                let sweep = self.sweep.as_ref().ok_or(AllocationError::InvalidDimensions)?;
                let phase = self.frame.activity.ok_or(AllocationError::InvalidDimensions)?;
                let width = if self.filled > 0.0 { self.filled } else { self.bar.width };
                let end = (*column + 4)
                    .min(((self.bar.x + width) * self.scale).ceil().min(self.width as f32) as u32);
                sweep_columns(
                    target,
                    sweep,
                    (self.bar.x, self.y, width, self.bar_height),
                    phase,
                    self.scale,
                    self.bar.width,
                    *column..end,
                );
                *column = end;
                if end >= ((self.bar.x + width) * self.scale).ceil().min(self.width as f32) as u32 {
                    self.stage = Stage::Done;
                }
            }
            Stage::Done => return Ok(true),
        }
        Ok(false)
    }
    /// Allocate the optional activity coverage only after dropping completed paint scratch.
    fn begin_sweep(&mut self) -> Result<(), AllocationError> {
        self.stage = Stage::Done;
        if self.frame.activity.is_some() {
            let width = if self.filled > 0.0 { self.filled } else { self.bar.width };
            if let Some(path) = self.bar.clip(width, self.scale) {
                self.stage =
                    Stage::SweepMask(CoverageRaster::new(&path, self.width, self.height, true)?);
            }
        }
        Ok(())
    }
}
/// Preserve the lookup table, fixed increment and column-major composition order.
fn sweep_columns(
    target: &mut Bitmap,
    clip: &CoverageMask,
    geometry: (f32, f32, f32, f32),
    phase: f32,
    scale: f32,
    track_width: f32,
    columns: std::ops::Range<u32>,
) {
    let (x, y, width, height) = geometry;
    let band = (f64::from(width) * 0.35).max(12.0);
    let center = f64::from(x) - band + f64::from(phase) * (f64::from(width) + 2.0 * band);
    let left = (x * scale).floor().max(0.0) as u32;
    let origin = (((f64::from(left) + 0.5) / f64::from(scale) - center + band) / (2.0 * band)
        * 1023.0
        * 256.0) as i64;
    let increment = (1023.0 * 256.0 / (2.0 * band * f64::from(scale))) as i64;
    let top = (y * scale).floor().max(0.0) as u32;
    let bottom = ((y + height) * scale).ceil().min(target.height() as f32) as u32;
    for column in columns {
        let position = origin + i64::from(column - left) * increment;
        let table_index = ((position + 128) >> 8).clamp(0, 1023) as f64;
        let table_position = (table_index + 0.5) / 1024.0;
        let weight = (512.0 * table_position.min(1.0 - table_position)).round() as u32;
        let alpha16 = 65 * 257 * weight / 256;
        let alpha = ((alpha16 + 128) / 257) as u8;
        for row in top..bottom {
            let index = (row * target.width() + column) as usize;
            let horizontal = ((column as f32 + 1.0).min((x + track_width) * scale)
                - (column as f32).max(x * scale))
            .clamp(0.0, 1.0);
            let vertical = ((row as f32 + 1.0).min((y + height) * scale)
                - (row as f32).max(y * scale))
            .clamp(0.0, 1.0);
            let fill_coverage = (horizontal * vertical * 255.0).floor() as u8;
            let coverage = byte_product(alpha, byte_product(clip.data()[index], fill_coverage));
            for channel in &mut target.data_mut()[index * 4..index * 4 + 4] {
                *channel = coverage + byte_product(*channel, 255 - coverage);
            }
        }
    }
}
