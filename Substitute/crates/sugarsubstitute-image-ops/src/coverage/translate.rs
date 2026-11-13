// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move coverage by whole pixels, as dragging a selection outline or a mask does.
use super::{Coverage, beyond};
use crate::{ImageError, Region, check};
use sugarsubstitute_contracts::execution::JobContext;

impl Coverage {
    /// The same coverage moved by whole document pixels. Content leaving the extent is kept
    /// beyond it, so moving it back restores it exactly; the area it uncovers is empty.
    /// # Errors
    /// Preserves cancellation, deadline and memory pressure.
    pub fn translate(&self, [dx, dy]: [i64; 2], context: &JobContext) -> Result<Self, ImageError> {
        check(context)?;
        if dx == 0 && dy == 0 {
            return Ok(self.clone());
        }
        let Some(frame) = self.content_bounds() else {
            return Self::new(self.extent, 0, context.memory());
        };
        let landed = [frame[0] + dx, frame[1] + dy, frame[2] + dx, frame[3] + dy];
        if self.within(beyond::rect(self.extent, [0, 0]))
            && beyond::intersection(landed, beyond::rect(self.extent, [0, 0])) == Some(landed)
        {
            return self.shift_inside([dx, dy], frame, context);
        }
        let whole = self.whole_in(frame, context.memory(), context)?;
        Self::from_whole(self.extent, &whole, [landed[0], landed[1]], context.memory(), context)
    }

    /// Move content that stays inside the extent by copying its bounds once.
    fn shift_inside(
        &self,
        [dx, dy]: [i64; 2],
        [left, top, right, bottom]: beyond::Frame,
        context: &JobContext,
    ) -> Result<Self, ImageError> {
        let mut moved = Self::new(self.extent, 0, context.memory())?;
        let source =
            Region::new(left as u32, top as u32, (right - left) as u32, (bottom - top) as u32)?;
        let destination =
            Region::new((left + dx) as u32, (top + dy) as u32, source.width(), source.height())?;
        let _claim = context.memory().claim(source.pixels() * 2 + 64)?;
        let mut values = Vec::new();
        values.try_reserve_exact(source.pixels() as usize)?;
        values.resize(source.pixels() as usize, 0);
        self.read(source, &mut values)?;
        check(context)?;
        moved.write(destination, &values, context.memory())?;
        Ok(moved)
    }
}
