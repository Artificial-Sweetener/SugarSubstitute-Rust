// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validated physical source indices for native display without GPU-dependent rounding.
use crate::GpuError;
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::{DisplaySample, Extent};

/// Borrowed fixed-point sampling map for one completed encoded display image.
/// Each output axis selects two source indices and an eight-bit interpolation weight.
/// Uncovered samples select the opaque matte.
/// The producer owns camera math. This adapter validates only bounded texture access.
#[derive(Clone, Copy)]
pub struct DisplayMapping<'a> {
    source: Extent,
    output: Extent,
    columns: &'a [DisplaySample],
    rows: &'a [DisplaySample],
    matte: [u8; 4],
}
impl<'a> DisplayMapping<'a> {
    /// Validate immutable indices and encoded RGBA matte before native allocation or writes.
    /// # Errors
    /// Rejects extents above 16384 per edge, incomplete maps, nonopaque matte or invalid indices.
    pub fn new(
        source: Extent,
        output: Extent,
        columns: &'a [DisplaySample],
        rows: &'a [DisplaySample],
        matte: [u8; 4],
    ) -> Result<Self, GpuError> {
        if [source.width(), source.height(), output.width(), output.height()]
            .iter()
            .any(|edge| *edge > 16384)
            || columns.len() != output.width() as usize
            || rows.len() != output.height() as usize
            || matte[3] != 255
            || columns.iter().any(|sample| !sample.is_valid_for(source.width()))
            || rows.iter().any(|sample| !sample.is_valid_for(source.height()))
        {
            return Err(GpuError::invalid("Invalid native display projection map"));
        }
        Ok(Self { source, output, columns, rows, matte })
    }
    /// Dimensions of the exact retained image supplying source pixels.
    pub const fn source(self) -> Extent {
        self.source
    }
    /// Physical output size, independent of retained source dimensions.
    pub const fn output(self) -> Extent {
        self.output
    }
}

/// Host encoding retains its original claim; submission separately admits native write staging.
pub(crate) struct MappingUpload {
    pub(crate) bytes: Vec<u8>,
    _memory: MemoryClaim,
}
impl MappingUpload {
    pub(crate) const CAPACITY: u64 = (8 + 6 * 16384) * 4;
    /// Encode one bounded little-endian mapping; identity presentation uploads only its header.
    pub(crate) fn new(
        mapping: Option<DisplayMapping<'_>>,
        output: Extent,
        context: &JobContext,
    ) -> Result<Self, GpuError> {
        crate::adapter::check(context)?;
        let words = 8 + mapping.map_or(0, |map| (map.columns.len() + map.rows.len()) * 3);
        let memory = context
            .memory()
            .claim(words as u64 * 4 + 128)
            .map_err(|cause| GpuError::new("gpu_display_mapping_admission", cause))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(words * 4)
            .map_err(|cause| GpuError::new("gpu_display_mapping_allocation", cause))?;
        let matte = mapping.map_or([0, 0, 0, 255], |map| map.matte);
        let background = u32::from_be_bytes([matte[3], matte[0], matte[1], matte[2]]);
        for word in
            [u32::from(mapping.is_some()), output.width(), output.height(), background, 0, 0, 0, 0]
        {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        if let Some(mapping) = mapping {
            for sample in mapping.columns.iter().chain(mapping.rows) {
                for word in [sample.first(), sample.second(), sample.weight()] {
                    bytes.extend_from_slice(&word.to_le_bytes());
                }
            }
        }
        Ok(Self { bytes, _memory: memory })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_mapping_rejects_incomplete_foreign_indices_and_transparency()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = Extent::new(3, 2)?;
        let output = Extent::new(2, 3)?;
        let matte = [17, 29, 41, 255];
        let columns = [DisplaySample::from_corner(1.0, 3, true), DisplaySample::UNCOVERED];
        let rows = [
            DisplaySample::from_corner(1.5, 2, true),
            DisplaySample::from_corner(0.5, 2, true),
            DisplaySample::UNCOVERED,
        ];
        let valid = DisplayMapping::new(source, output, &columns, &rows, matte)?;
        assert_eq!(valid.source(), source);
        assert_eq!(valid.output(), output);
        let foreign_column = [DisplaySample::from_corner(3.5, 4, false), columns[0]];
        let foreign_row = [rows[0], DisplaySample::from_corner(2.5, 3, false), rows[2]];
        assert!(DisplayMapping::new(source, output, &foreign_column, &rows, matte).is_err());
        assert!(DisplayMapping::new(source, output, &columns, &foreign_row, matte).is_err());
        assert!(DisplayMapping::new(source, output, &columns[..1], &rows, matte).is_err());
        assert!(DisplayMapping::new(source, output, &columns, &rows, [0; 4]).is_err());
        assert!(DisplayMapping::new(source, Extent::new(16385, 1)?, &[], &[], matte).is_err());
        Ok(())
    }
}
