// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Disposable bilinear or nearest display pixels sampled from a canonical retained viewport.
use crate::{CpuSurface, RenderError, cpu::check};
use sugarsubstitute_canvas::{FrameStamp, ViewProjection};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim, MemoryLease};
use sugarsubstitute_image_ops::{
    DisplaySample, ImageError, Pixel, SrgbEncoder, bilinear_display_pixel,
};

/// Admitted physical source samples shared by software and native GPU display projection.
/// Uncovered entries receive the display matte; covered entries address canonical pixels.
pub struct ProjectionSamples {
    view: ViewProjection,
    columns: Vec<DisplaySample>,
    rows: Vec<DisplaySample>,
    _memory: MemoryClaim,
}
impl ProjectionSamples {
    /// Build bounded retained-source samples on an admitted worker, without image evaluation.
    /// Cameras through 200% use bilinear encoded-pixel display interpolation; larger
    /// magnification preserves exact nearest-neighbor source pixels.
    /// # Errors
    /// Preserves original allocation/admission failures and observes cancellation/deadline.
    pub fn new(view: ViewProjection, context: &JobContext) -> Result<Self, RenderError> {
        check(context)?;
        let source = view.source().surface();
        let target = view.target().surface();
        let bytes = (u64::from(target.width()) + u64::from(target.height()))
            * size_of::<DisplaySample>() as u64
            + 256;
        let memory = context.memory().claim(bytes).map_err(ImageError::from)?;
        let mut columns = Vec::new();
        let mut rows = Vec::new();
        columns.try_reserve_exact(target.width() as usize).map_err(ImageError::from)?;
        rows.try_reserve_exact(target.height() as usize).map_err(ImageError::from)?;
        let smooth = view.target().zoom() <= 2.0;
        columns.extend((0..target.width()).map(|x| {
            view.column_position(x).map_or(DisplaySample::UNCOVERED, |position| {
                DisplaySample::from_corner(position, source.width(), smooth)
            })
        }));
        rows.extend((0..target.height()).map(|y| {
            view.row_position(y).map_or(DisplaySample::UNCOVERED, |position| {
                DisplaySample::from_corner(position, source.height(), smooth)
            })
        }));
        check(context)?;
        Ok(Self { view, columns, rows, _memory: memory })
    }
    /// Physical source and target cameras used to construct every index.
    pub const fn view(&self) -> ViewProjection {
        self.view
    }
    /// One canonical source sample per output column.
    pub fn columns(&self) -> &[DisplaySample] {
        &self.columns
    }
    /// One canonical source sample per output row.
    pub fn rows(&self) -> &[DisplaySample] {
        &self.rows
    }
    /// Sample one projected opaque encoded pixel from the retained canonical source.
    /// Projection construction already validated extents; callers provide in-range target pixels.
    pub fn sample_pixel(&self, source: &CpuSurface, x: u32, y: u32, matte: u32) -> u32 {
        let horizontal = self.columns[x as usize];
        let vertical = self.rows[y as usize];
        if !horizontal.is_covered() || !vertical.is_covered() {
            return matte;
        }
        let stride = source.extent().width() as usize;
        let pixels = source.pixels();
        let index = |column: u32, row: u32| row as usize * stride + column as usize;
        if horizontal.weight() == 0 && vertical.weight() == 0 {
            return pixels[index(horizontal.first(), vertical.first())];
        }
        bilinear_display_pixel(
            [
                pixels[index(horizontal.first(), vertical.first())],
                pixels[index(horizontal.second(), vertical.first())],
                pixels[index(horizontal.first(), vertical.second())],
                pixels[index(horizontal.second(), vertical.second())],
            ],
            horizontal,
            vertical,
        )
    }
    /// Borrow the same exact index arrays for native GPU presentation; no display pixel readback.
    /// # Errors
    /// Rejects nonopaque matte and preserves original color/graphics validation errors.
    pub fn gpu_mapping(
        &self,
        matte: Pixel,
    ) -> Result<sugarsubstitute_image_gpu::DisplayMapping<'_>, RenderError> {
        Ok(sugarsubstitute_image_gpu::DisplayMapping::new(
            self.view.source().surface(),
            self.view.target().surface(),
            &self.columns,
            &self.rows,
            SrgbEncoder::encode_constant(matte)?,
        )?)
    }
}

/// Immutable approximate display pixels with a separate target and canonical source identity.
/// This type cannot be applied to a `CpuSurface` or used as a prepared exact repair.
pub struct CpuProjection {
    target: FrameStamp,
    source: FrameStamp,
    view: ViewProjection,
    buffer: ProjectionBuffer,
}

/// Exclusive unpublished display storage. Recycling consumes the preceding projection identity;
/// no preview pixels become a source or exact repair dependency. Keep this value on workers.
pub struct ProjectionBuffer {
    extent: sugarsubstitute_image_ops::Extent,
    pixels: Vec<u32>,
    _memory: MemoryClaim,
}
/// Paired exclusive CPU projection allocations retained by one renderer reservation.
/// One buffer may remain in native presentation while the other is filled on its worker.
pub struct ProjectionBuffers {
    first: ProjectionBuffer,
    second: ProjectionBuffer,
}
impl ProjectionBuffers {
    /// Bind two separately admitted allocations into one exclusive handoff value.
    pub(crate) fn new(first: ProjectionBuffer, second: ProjectionBuffer) -> Self {
        Self { first, second }
    }
    /// Consume the pair into its two independently owned reusable allocations.
    pub fn into_parts(self) -> (ProjectionBuffer, ProjectionBuffer) {
        (self.first, self.second)
    }
}
impl ProjectionBuffer {
    /// Admit one output-sized allocation from the retained presentation lease on this worker.
    /// Control remains with the current job; recycling never creates another reservation.
    /// # Errors
    /// Preserves original allocation/admission/control failures before any frame is published.
    pub fn new(
        extent: sugarsubstitute_image_ops::Extent,
        memory: &MemoryLease,
        context: &JobContext,
    ) -> Result<Self, RenderError> {
        check(context)?;
        let memory = memory
            .claim(extent.pixels() * 4 + size_of::<CpuProjection>() as u64 + 128)
            .map_err(ImageError::from)?;
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(extent.pixels() as usize).map_err(ImageError::from)?;
        Ok(Self { extent, pixels, _memory: memory })
    }
    /// Physical storage dimensions; changed native sizes require a newly admitted allocation.
    pub const fn extent(&self) -> sugarsubstitute_image_ops::Extent {
        self.extent
    }
    /// Project canonical encoded pixels into one output-sized buffer on an admitted worker.
    /// Uses physical pixel centers, safe matte gaps, contiguous copies for integer pans, and
    /// row reuse while magnifying. No DAG evaluation, display-table allocation or native call runs.
    /// # Errors
    /// Rejects mismatched extents/identities, stale targets and nonopaque matte; preserves
    /// original allocation/admission/cancellation/deadline failures without changing the source.
    pub fn project(
        mut self,
        target: FrameStamp,
        source: &CpuSurface,
        view: ViewProjection,
        matte: Pixel,
        context: &JobContext,
    ) -> Result<CpuProjection, RenderError> {
        check(context)?;
        let base = source.current().ok_or(RenderError::StaleBase)?;
        if view.source().surface() != source.extent()
            || target.surface != base.surface
            || view.target().surface() != self.extent
        {
            return Err(RenderError::Invalid("Projection source extent or surface differs"));
        }
        if target.generation <= base.generation {
            return Err(RenderError::StaleTarget);
        }
        let [red, green, blue, alpha] = SrgbEncoder::encode_constant(matte)?;
        let background = u32::from_be_bytes([alpha, red, green, blue]);
        let samples = ProjectionSamples::new(view, context)?;
        self.pixels.clear();
        project(source, &samples, background, &mut self.pixels, context)?;
        check(context)?;
        Ok(CpuProjection { target, source: base, view, buffer: self })
    }
}
impl CpuProjection {
    /// Consume the display identity and recover only its exclusive allocation for worker reuse.
    pub fn into_buffer(self) -> ProjectionBuffer {
        self.buffer
    }
    /// Distinct application-assigned display generation; it never changes canonical backing.
    pub const fn target(&self) -> FrameStamp {
        self.target
    }
    /// Actual canonical generation sampled by this immutable result.
    pub const fn source(&self) -> FrameStamp {
        self.source
    }
    /// Actual source and approximate output cameras represented by these pixels.
    pub const fn view(&self) -> ViewProjection {
        self.view
    }
    /// Opaque top-row-first sRGB `0xffRRGGBB` words with output-viewport dimensions.
    pub fn pixels(&self) -> &[u32] {
        &self.buffer.pixels
    }
}

/// Sample visible columns once per distinct source row; never read a previous projection.
fn project(
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

#[cfg(test)]
mod tests;
