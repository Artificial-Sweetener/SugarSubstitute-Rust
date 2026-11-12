// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exchange coverage as 8-bit values, the precision of the canvas protocol and of mask files.
//!
//! Widening multiplies by 257, so every 8-bit value round-trips exactly; narrowing rounds to
//! the nearest 8-bit value.
use super::Coverage;
use crate::{Extent, ImageError, check};
use sugarsubstitute_contracts::execution::JobContext;

/// Widen one 8-bit coverage value.
pub const fn coverage_from_u8(value: u8) -> u16 {
    value as u16 * 257
}

/// Narrow one coverage value to the nearest 8-bit value.
pub const fn coverage_to_u8(value: u16) -> u8 {
    ((value as u32 * 255 + 32_767) / 65_535) as u8
}

impl Coverage {
    /// Coverage from row-major 8-bit values over `extent`.
    /// # Errors
    /// Rejects a wrong value count and preserves cancellation and memory pressure.
    pub fn from_coverage8(
        extent: Extent,
        values: &[u8],
        context: &JobContext,
    ) -> Result<Self, ImageError> {
        if values.len() as u64 != extent.pixels() {
            return Err(ImageError::Invalid("Coverage values differ from their extent"));
        }
        let mut coverage = Self::new(extent, 0, context.memory())?;
        let width = extent.width() as usize;
        coverage.update_in_job(extent.region(), context, |x, y, _| {
            coverage_from_u8(values[y as usize * width + x as usize])
        })?;
        Ok(coverage)
    }

    /// Row-major 8-bit values over the whole extent.
    /// # Errors
    /// Preserves cancellation and allocation failure.
    pub fn to_coverage8(&self, context: &JobContext) -> Result<Vec<u8>, ImageError> {
        let mut output = Vec::new();
        output.try_reserve_exact(self.extent.pixels() as usize)?;
        output.resize(self.extent.pixels() as usize, 0);
        let width = self.extent.width() as usize;
        let mut band_values = Vec::new();
        for band in self.extent.region().tiles() {
            check(context)?;
            band_values.clear();
            band_values.try_reserve_exact(band.pixels() as usize)?;
            band_values.resize(band.pixels() as usize, 0);
            self.read(band, &mut band_values)?;
            for (row, values) in band_values.chunks_exact(band.width() as usize).enumerate() {
                let start = (band.y() as usize + row) * width + band.x() as usize;
                for (target, value) in output[start..start + values.len()].iter_mut().zip(values) {
                    *target = coverage_to_u8(*value);
                }
            }
        }
        Ok(output)
    }
}
