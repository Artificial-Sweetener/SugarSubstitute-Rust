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

/// Most overlay pixels one display draw carries.
pub const MAX_OVERLAY_PIXELS: usize = 1 << 17;

/// One opaque device pixel drawn over a display, such as a dash of selection outline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverlayPixel {
    /// Output column.
    pub x: u32,
    /// Output row.
    pub y: u32,
    /// Encoded sRGB colour.
    pub color: [u8; 3],
}

/// Host encoding retains its original claim; submission separately admits native write staging.
pub(crate) struct MappingUpload {
    pub(crate) bytes: Vec<u8>,
    /// Overlay pixels drawn after the display, one point each.
    pub(crate) overlay: u32,
    _memory: MemoryClaim,
}
impl MappingUpload {
    pub(crate) const CAPACITY: u64 = (8 + 12 * 16384 + 2 * MAX_OVERLAY_PIXELS as u64) * 4;
    /// Encode bounded little-endian primary and optional underlay mappings, then the overlay
    /// pixels drawn over them. Identity presentation uploads only its header and overlay.
    pub(crate) fn new(
        mapping: Option<DisplayMapping<'_>>,
        underlay: Option<DisplayMapping<'_>>,
        overlay: &[OverlayPixel],
        output: Extent,
        context: &JobContext,
    ) -> Result<Self, GpuError> {
        crate::adapter::check(context)?;
        if overlay.len() > MAX_OVERLAY_PIXELS
            || overlay.iter().any(|pixel| pixel.x >= output.width() || pixel.y >= output.height())
        {
            return Err(GpuError::invalid("Display overlay pixels exceed their output"));
        }
        let layout = MappingLayout::new(mapping, underlay, output, overlay.len())?;
        let memory = context
            .memory()
            .claim(layout.words as u64 * 4 + 128)
            .map_err(|cause| GpuError::new("gpu_display_mapping_admission", cause))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(layout.words * 4)
            .map_err(|cause| GpuError::new("gpu_display_mapping_allocation", cause))?;
        encode_mappings(&mut bytes, mapping, underlay, output, layout);
        for pixel in overlay {
            let [red, green, blue] = pixel.color;
            for word in [pixel.x | pixel.y << 16, u32::from_be_bytes([0, red, green, blue])] {
                bytes.extend_from_slice(&word.to_le_bytes());
            }
        }
        Ok(Self { bytes, overlay: overlay.len() as u32, _memory: memory })
    }
}

#[derive(Clone, Copy)]
struct MappingLayout {
    words: usize,
    underlay_base: usize,
    overlay_base: usize,
    overlay: usize,
}
impl MappingLayout {
    fn new(
        mapping: Option<DisplayMapping<'_>>,
        underlay: Option<DisplayMapping<'_>>,
        output: Extent,
        overlay: usize,
    ) -> Result<Self, GpuError> {
        if mapping.is_some_and(|map| map.output != output)
            || underlay.is_some_and(|map| map.output != output)
            || underlay.is_some() && mapping.is_none()
        {
            return Err(GpuError::invalid("Native display mappings disagree on output extent"));
        }
        let primary_words = mapping.map_or(0, axis_words);
        let underlay_base = underlay.map_or(0, |_| 8 + primary_words);
        let overlay_base = 8 + primary_words + underlay.map_or(0, axis_words);
        let words = overlay_base + 2 * overlay;
        if words as u64 * 4 > MappingUpload::CAPACITY {
            return Err(GpuError::invalid("Native display mappings exceed upload capacity"));
        }
        Ok(Self { words, underlay_base, overlay_base, overlay })
    }
}

fn axis_words(mapping: DisplayMapping<'_>) -> usize {
    (mapping.columns.len() + mapping.rows.len()) * 3
}

fn encode_mappings(
    bytes: &mut Vec<u8>,
    mapping: Option<DisplayMapping<'_>>,
    underlay: Option<DisplayMapping<'_>>,
    output: Extent,
    layout: MappingLayout,
) {
    let matte = mapping.map_or([0, 0, 0, 255], |map| map.matte);
    let background = u32::from_be_bytes([matte[3], matte[0], matte[1], matte[2]]);
    for word in [
        u32::from(mapping.is_some()),
        output.width(),
        output.height(),
        background,
        layout.underlay_base as u32,
        layout.overlay_base as u32,
        layout.overlay as u32,
        0,
    ] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    for mapping in [mapping, underlay].into_iter().flatten() {
        for sample in mapping.columns.iter().chain(mapping.rows) {
            for word in [sample.first(), sample.second(), sample.weight()] {
                bytes.extend_from_slice(&word.to_le_bytes());
            }
        }
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

    #[test]
    fn composited_mapping_encodes_underlay_at_bounded_header_offset()
    -> Result<(), Box<dyn std::error::Error>> {
        let primary_source = Extent::new(3, 2)?;
        let underlay_source = Extent::new(5, 4)?;
        let output = Extent::new(2, 1)?;
        let primary_columns = [DisplaySample::from_corner(0.25, 3, true), DisplaySample::UNCOVERED];
        let primary_rows = [DisplaySample::from_corner(1.0, 2, true)];
        let underlay_columns =
            [DisplaySample::from_corner(2.5, 5, true), DisplaySample::from_corner(3.25, 5, true)];
        let underlay_rows = [DisplaySample::from_corner(1.5, 4, true)];
        let primary = DisplayMapping::new(
            primary_source,
            output,
            &primary_columns,
            &primary_rows,
            [17, 29, 41, 255],
        )?;
        let underlay = DisplayMapping::new(
            underlay_source,
            output,
            &underlay_columns,
            &underlay_rows,
            [17, 29, 41, 255],
        )?;
        let layout = MappingLayout::new(Some(primary), Some(underlay), output, 0)?;
        assert_eq!(layout.underlay_base, 17);
        assert_eq!(layout.words, 26);
        let mut bytes = Vec::new();
        encode_mappings(&mut bytes, Some(primary), Some(underlay), output, layout);
        let words = bytes
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
            .collect::<Vec<_>>();
        assert_eq!(&words[..8], &[1, 2, 1, 0xff111d29, 17, 26, 0, 0]);
        assert_eq!(
            &words[17..],
            &[
                underlay_columns[0].first(),
                underlay_columns[0].second(),
                underlay_columns[0].weight(),
                underlay_columns[1].first(),
                underlay_columns[1].second(),
                underlay_columns[1].weight(),
                underlay_rows[0].first(),
                underlay_rows[0].second(),
                underlay_rows[0].weight(),
            ]
        );
        let different_output = Extent::new(1, 1)?;
        assert!(MappingLayout::new(Some(primary), Some(underlay), different_output, 0).is_err());
        assert!(MappingLayout::new(None, Some(underlay), output, 0).is_err());
        Ok(())
    }
}
