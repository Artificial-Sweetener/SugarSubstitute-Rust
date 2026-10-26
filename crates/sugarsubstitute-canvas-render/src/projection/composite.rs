// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose retained canonical coverage without making disposable pixels authoritative.
use super::*;

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
        self.pixels.clear();
        project_primary(source, &samples, background, &mut self.pixels, context)?;
        if let (Some((surface, _)), Some(fallback_samples)) = (underlay, fallback_samples.as_ref())
        {
            fill_uncovered(surface, &samples, fallback_samples, &mut self.pixels, context)?;
        }
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

/// Sample visible primary columns once per distinct source row.
fn project_primary(
    source: &CpuSurface,
    samples: &ProjectionSamples,
    matte: u32,
    output: &mut Vec<u32>,
    context: &JobContext,
) -> Result<(), RenderError> {
    let columns = samples.columns();
    let width = columns.len();
    let first = columns.iter().position(|column| column.is_covered()).unwrap_or(width);
    let end =
        columns.iter().rposition(|column| column.is_covered()).map_or(first, |index| index + 1);
    let visible = &columns[first..end];
    let contiguous = !visible.is_empty()
        && visible.iter().all(|sample| sample.weight() == 0)
        && visible.windows(2).all(|pair| pair[1].first() == pair[0].first() + 1);
    let stride = source.extent().width() as usize;
    let mut previous = None;
    for (y, &row) in samples.rows().iter().enumerate() {
        check(context)?;
        let start = output.len();
        if previous == Some(row) {
            output.extend_from_within(start - width..start);
        } else if !row.is_covered() || visible.is_empty() {
            output.resize(start + width, matte);
        } else if row.weight() == 0 && contiguous {
            let source_row = &source.pixels()
                [row.first() as usize * stride..(row.first() as usize + 1) * stride];
            output.resize(start + first, matte);
            let begin = visible[0].first() as usize;
            output.extend_from_slice(&source_row[begin..begin + visible.len()]);
            output.resize(start + width, matte);
        } else {
            output.resize(start + first, matte);
            output.extend(
                (first..end).map(|x| samples.sample_pixel(source, x as u32, y as u32, matte)),
            );
            output.resize(start + width, matte);
        }
        previous = Some(row);
    }
    Ok(())
}

fn fill_uncovered(
    source: &CpuSurface,
    primary: &ProjectionSamples,
    fallback: &ProjectionSamples,
    output: &mut [u32],
    context: &JobContext,
) -> Result<(), RenderError> {
    let width = primary.columns().len();
    let first = primary.columns().iter().position(|sample| sample.is_covered()).unwrap_or(width);
    let end = primary
        .columns()
        .iter()
        .rposition(|sample| sample.is_covered())
        .map_or(first, |index| index + 1);
    for y in 0..primary.rows().len() {
        check(context)?;
        if !fallback.rows()[y].is_covered() {
            continue;
        }
        let gaps =
            if primary.rows()[y].is_covered() { [0..first, end..width] } else { [0..width, 0..0] };
        for gap in gaps {
            for x in gap {
                if fallback.columns()[x].is_covered() {
                    output[y * width + x] = fallback.sample_pixel(source, x as u32, y as u32, 0);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
