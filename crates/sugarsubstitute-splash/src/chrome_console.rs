// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain the console's thirteen independently composited passes across bounded yields.

use super::{border_raster::BorderCursor, console_passes};
use crate::{
    Appearance,
    shapes::{AllocationError, PaintCursor},
};
use tiny_skia::{Path, Pixmap, Rect};

pub(super) enum ConsolePass {
    Fill(Path, [u8; 4]),
    Arc([[f64; 2]; 4], [u8; 4], Rect),
}
enum ActivePass {
    Fill(PaintCursor),
    Arc(BorderCursor),
}
/// Own coverage and pass order without borrowing a target through a yield.
pub(crate) struct ConsoleCursor {
    passes: std::vec::IntoIter<ConsolePass>,
    active: Option<ActivePass>,
    width: u32,
    height: u32,
    scale: f32,
}
impl ConsoleCursor {
    /// Capture fixed console geometry without clearing or integrating target-sized masks.
    pub(crate) fn new(
        width: u32,
        height: u32,
        appearance: Appearance,
        scale: f32,
    ) -> Result<Self, AllocationError> {
        Ok(Self {
            passes: console_passes(appearance, scale)?.into_iter(),
            active: None,
            width,
            height,
            scale,
        })
    }
    /// Advance at most one bounded coverage/composition slice in source paint order.
    pub(crate) fn step(&mut self, target: &mut Pixmap) -> Result<bool, AllocationError> {
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
