// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move coverage by whole pixels, as dragging a selection outline does.
use super::Coverage;
use crate::{ImageError, Region, check};
use sugarsubstitute_contracts::execution::JobContext;

impl Coverage {
    /// The same coverage moved by whole document pixels. Coverage leaving the extent is dropped
    /// and the area it uncovers is empty, including where untouched tiles were covered.
    /// # Errors
    /// Preserves cancellation, deadline and memory pressure.
    pub fn translate(&self, [dx, dy]: [i64; 2], context: &JobContext) -> Result<Self, ImageError> {
        check(context)?;
        if dx == 0 && dy == 0 {
            return Ok(self.clone());
        }
        let mut moved = Self::new(self.extent, 0, context.memory())?;
        let Some(bounds) = self.bounds() else {
            return Ok(moved);
        };
        let Some((left, right)) = landing(bounds.x(), bounds.right(), dx, self.extent.width())
        else {
            return Ok(moved);
        };
        let Some((top, bottom)) = landing(bounds.y(), bounds.bottom(), dy, self.extent.height())
        else {
            return Ok(moved);
        };
        let source = Region::new(left, top, right - left, bottom - top)?;
        let destination =
            Region::new(shifted(left, dx)?, shifted(top, dy)?, source.width(), source.height())?;
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

/// The source span `[start, end)` whose pixels still land inside `[0, limit)` after `delta`.
fn landing(start: u32, end: u32, delta: i64, limit: u32) -> Option<(u32, u32)> {
    let low = i64::from(start).max(-delta);
    let high = i64::from(end).min(i64::from(limit) - delta);
    (low < high).then_some((low as u32, high as u32))
}

fn shifted(value: u32, delta: i64) -> Result<u32, ImageError> {
    u32::try_from(i64::from(value) + delta)
        .map_err(|_| ImageError::Invalid("Translated coverage leaves its extent"))
}
