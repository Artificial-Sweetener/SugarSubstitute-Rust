// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain the console's thirteen independently composited passes across bounded yields.

use super::{OutputPanel, border::BorderCursor, panel_passes};
use crate::bitmap::Bitmap;
use crate::shapes::{AllocationError, PaintCursor};
use tiny_skia::{Path, Rect};

pub(super) enum ConsolePass {
    Fill(Path, [u8; 4]),
    Arc([[f64; 2]; 4], [u8; 4], Rect),
}
enum ActivePass {
    Fill(PaintCursor),
    Arc(BorderCursor),
}
/// Own coverage and pass order without borrowing a target through a yield.
pub struct PanelCursor {
    passes: std::vec::IntoIter<ConsolePass>,
    active: Option<ActivePass>,
    width: u32,
    height: u32,
    scale: f32,
}
impl PanelCursor {
    /// Capture validated panel geometry without clearing or integrating target-sized masks.
    /// # Errors
    /// Rejects invalid target geometry, panel bounds and device scale before allocation.
    pub fn new(
        width: u32,
        height: u32,
        panel: OutputPanel,
        scale: f32,
    ) -> Result<Self, AllocationError> {
        panel.validate(scale)?;
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err(AllocationError::InvalidDimensions);
        }
        Ok(Self {
            passes: panel_passes(panel, scale)?.into_iter(),
            active: None,
            width,
            height,
            scale,
        })
    }
    /// Advance at most one bounded coverage/composition slice in source paint order.
    /// # Errors
    /// Preserves coverage allocation failures and rejects a changed target shape.
    pub fn step(&mut self, target: &mut Bitmap) -> Result<bool, AllocationError> {
        if target.width() != self.width || target.height() != self.height {
            return Err(AllocationError::InvalidDimensions);
        }
        if let Some(active) = &mut self.active {
            let done = match active {
                ActivePass::Fill(cursor) => cursor.step(target, None, 32768)?,
                ActivePass::Arc(cursor) => cursor.step(target)?,
            };
            if done {
                self.active = None;
            }
            return Ok(false);
        }
        let Some(pass) = self.passes.next() else { return Ok(true) };
        self.active = Some(match pass {
            ConsolePass::Fill(path, color) => ActivePass::Fill(PaintCursor::new(
                &path,
                self.width,
                self.height,
                color,
                None,
                true,
            )?),
            ConsolePass::Arc(points, color, clip) => ActivePass::Arc(BorderCursor::new(
                self.width,
                self.height,
                points,
                color,
                self.scale,
                clip,
            )?),
        });
        Ok(false)
    }
}
