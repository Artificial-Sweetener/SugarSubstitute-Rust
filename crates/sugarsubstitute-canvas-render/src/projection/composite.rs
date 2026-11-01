// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose retained canonical coverage without making disposable pixels authoritative.
use super::*;
use sugarsubstitute_image_ops::bilinear_display_row;

/// Output rows per independently prepared band; CPU jobs spread bands over helper threads.
const BAND_ROWS: usize = 32;

impl ProjectionBuffer {
    /// Project a primary canonical surface and fill only its uncovered output pixels from an
    /// optional canonical underlay. Pixels uncovered by both sources receive the opaque matte.
    /// The primary contiguous-copy and repeated-row fast paths remain authoritative in overlap.
    /// # Errors
    /// Rejects mismatched target cameras, extents, surfaces, stale generations or nonopaque matte;
    /// preserves admission, cancellation and deadline failures without changing either source.
    pub fn project_over(
        mut self,
        target: FrameStamp,
        source: &CpuSurface,
        view: ViewProjection,
        underlay: Option<(&CpuSurface, ViewProjection)>,
        matte: Pixel,
        context: &JobContext,
    ) -> Result<CpuProjection, RenderError> {
        check(context)?;
        let primary = validate_source(target, self.extent, source, view)?;
        let fallback = underlay
            .map(|(surface, mapping)| {
                if mapping.target() != view.target() {
                    return Err(RenderError::Invalid("Projection target cameras differ"));
                }
                validate_source(target, self.extent, surface, mapping)
            })
            .transpose()?;
        let [red, green, blue, alpha] = SrgbEncoder::encode_constant(matte)?;
        let background = u32::from_be_bytes([alpha, red, green, blue]);
        let samples = ProjectionSamples::new(view, context)?;
        let fallback_samples =
            underlay.map(|(_, mapping)| ProjectionSamples::new(mapping, context)).transpose()?;
        let rows = RowProjection::new(
            source,
            &samples,
            underlay.map(|(surface, _)| surface).zip(fallback_samples.as_ref()),
            background,
        );
        let pixels = self.extent.pixels() as usize;
        if self.pixels.len() != pixels {
            // Recycled storage keeps its length; every pixel is overwritten below.
            self.pixels.clear();
            self.pixels.resize(pixels, background);
        }
        let width = self.extent.width() as usize;
        context.try_for_each_chunk_mut(&mut self.pixels, BAND_ROWS * width, |band, pixels| {
            rows.project(band * BAND_ROWS, pixels, context)
        })?;
        check(context)?;
        Ok(CpuProjection { target, source: primary, underlay: fallback, view, buffer: self })
    }
}

fn validate_source(
    target: FrameStamp,
    extent: sugarsubstitute_image_ops::Extent,
    source: &CpuSurface,
    view: ViewProjection,
) -> Result<FrameStamp, RenderError> {
    let current = source.current().ok_or(RenderError::StaleBase)?;
    if view.source().surface() != source.extent()
        || view.target().surface() != extent
        || target.surface != current.surface
    {
        return Err(RenderError::Invalid("Projection source extent or surface differs"));
    }
    if target.generation <= current.generation {
        return Err(RenderError::StaleTarget);
    }
    Ok(current)
}

/// Independent output-row projection of a primary canonical source over an optional underlay.
/// Any contiguous band of rows can be prepared separately into its own disjoint storage.
struct RowProjection<'a> {
    source: &'a CpuSurface,
    samples: &'a ProjectionSamples,
    underlay: Option<(&'a CpuSurface, &'a ProjectionSamples)>,
    matte: u32,
    /// First and one-past-last primary-covered output columns.
    visible: std::ops::Range<usize>,
    /// Whether visible columns address consecutive source pixels without interpolation.
    contiguous: bool,
}

impl<'a> RowProjection<'a> {
    fn new(
        source: &'a CpuSurface,
        samples: &'a ProjectionSamples,
        underlay: Option<(&'a CpuSurface, &'a ProjectionSamples)>,
        matte: u32,
    ) -> Self {
        let columns = samples.columns();
        let width = columns.len();
        let first = columns.iter().position(|column| column.is_covered()).unwrap_or(width);
        let end =
            columns.iter().rposition(|column| column.is_covered()).map_or(first, |index| index + 1);
        let visible = &columns[first..end];
        let contiguous = !visible.is_empty()
            && visible.iter().all(|sample| sample.weight() == 0)
            && visible.windows(2).all(|pair| pair[1].first() == pair[0].first() + 1);
        Self { source, samples, underlay, matte, visible: first..end, contiguous }
    }

    /// Fill `band`, whose first row is output row `first_row`, observing worker controls per row.
    fn project(
        &self,
        first_row: usize,
        band: &mut [u32],
        context: &JobContext,
    ) -> Result<(), RenderError> {
        let width = self.samples.columns().len();
        for offset in 0..band.len() / width {
            check(context)?;
            let y = first_row + offset;
            let start = offset * width;
            if offset > 0 && self.repeats_previous(y) {
                band.copy_within(start - width..start, start);
                continue;
            }
            let row = &mut band[start..start + width];
            self.project_primary(y, row);
            if let Some((surface, fallback)) = self.underlay {
                self.fill_uncovered(surface, fallback, y, row);
            }
        }
        Ok(())
    }

    /// Whether output row `y` samples exactly the same source rows as the row above it.
    fn repeats_previous(&self, y: usize) -> bool {
        let same = |samples: &ProjectionSamples| samples.rows()[y] == samples.rows()[y - 1];
        same(self.samples) && self.underlay.is_none_or(|(_, fallback)| same(fallback))
    }

    fn project_primary(&self, y: usize, row: &mut [u32]) {
        let vertical = self.samples.rows()[y];
        if !vertical.is_covered() || self.visible.is_empty() {
            row.fill(self.matte);
            return;
        }
        let stride = self.source.extent().width() as usize;
        let source_row = |index: u32| &self.source.pixels()[index as usize * stride..][..stride];
        let top = source_row(vertical.first());
        row[..self.visible.start].fill(self.matte);
        row[self.visible.end..].fill(self.matte);
        let columns = &self.samples.columns()[self.visible.clone()];
        let output = &mut row[self.visible.clone()];
        if vertical.weight() == 0 && self.contiguous {
            let begin = columns[0].first() as usize;
            output.copy_from_slice(&top[begin..begin + columns.len()]);
        } else {
            let bottom = source_row(vertical.second());
            bilinear_display_row(top, bottom, vertical, columns, self.matte, output);
        }
    }

    /// Fill only output pixels outside primary coverage from the canonical underlay.
    fn fill_uncovered(
        &self,
        surface: &CpuSurface,
        fallback: &ProjectionSamples,
        y: usize,
        row: &mut [u32],
    ) {
        let vertical = fallback.rows()[y];
        if !vertical.is_covered() {
            return;
        }
        let width = row.len();
        let gaps = if self.samples.rows()[y].is_covered() {
            [0..self.visible.start, self.visible.end..width]
        } else {
            [0..width, 0..0]
        };
        let stride = surface.extent().width() as usize;
        let source_row = |index: u32| &surface.pixels()[index as usize * stride..][..stride];
        let (top, bottom) = (source_row(vertical.first()), source_row(vertical.second()));
        for gap in gaps {
            // Uncovered underlay columns keep the matte the primary pass already wrote.
            bilinear_display_row(
                top,
                bottom,
                vertical,
                &fallback.columns()[gap.clone()],
                self.matte,
                &mut row[gap],
            );
        }
    }
}

#[cfg(test)]
mod tests;
