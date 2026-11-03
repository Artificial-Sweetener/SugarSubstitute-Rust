// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Disposable bilinear or nearest display pixels sampled from a canonical retained viewport.
mod composite;
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
    /// Whether one output pixel is covered by both canonical source axes.
    pub fn covers_pixel(&self, x: u32, y: u32) -> bool {
        self.columns.get(x as usize).is_some_and(|sample| sample.is_covered())
            && self.rows.get(y as usize).is_some_and(|sample| sample.is_covered())
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

/// Immutable approximate display pixels with separate target, primary and optional underlay
/// canonical identities.
/// This type cannot be applied to a `CpuSurface` or used as a prepared exact repair.
pub struct CpuProjection {
    target: FrameStamp,
    source: FrameStamp,
    underlay: Option<FrameStamp>,
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
        self,
        target: FrameStamp,
        source: &CpuSurface,
        view: ViewProjection,
        matte: Pixel,
        context: &JobContext,
    ) -> Result<CpuProjection, RenderError> {
        self.project_over(target, source, view, None, matte, context)
    }
}
impl CpuProjection {
    /// Consume these immutable display pixels under a newer presentation identity on the same
    /// surface. Source identities, cameras, physical pixel storage and its memory claim are moved
    /// unchanged in constant time, without allocation or image evaluation.
    /// The application assigns a target newer than every frame already accepted for the surface;
    /// this operation only compares against this projection's preceding target.
    /// Call where releasing this projection's storage on rejection is permitted.
    /// # Errors
    /// Returns [`RenderError::Invalid`] for a different surface and
    /// [`RenderError::StaleTarget`] for an equal or regressed generation. Rejection consumes and
    /// releases this projection and its exclusive allocation.
    pub fn reidentify(mut self, target: FrameStamp) -> Result<Self, RenderError> {
        self.reidentify_in_place(target)?;
        Ok(self)
    }
    /// Assign a newer presentation identity on the same surface while retaining exclusive custody.
    /// Source identities, cameras, physical pixels and their memory claim remain unchanged.
    /// This constant-time operation performs no allocation, evaluation or storage release.
    /// The application assigns a target newer than every frame already accepted for the surface;
    /// validation only compares against this projection's preceding target.
    /// # Errors
    /// Returns [`RenderError::Invalid`] for a different surface and
    /// [`RenderError::StaleTarget`] for an equal or regressed generation. Every error preserves
    /// the preceding target and complete pixel allocation for publication or worker retirement.
    pub fn reidentify_in_place(&mut self, target: FrameStamp) -> Result<(), RenderError> {
        if target.surface != self.target.surface {
            return Err(RenderError::Invalid("Projection target surface differs"));
        }
        if target.generation <= self.target.generation {
            return Err(RenderError::StaleTarget);
        }
        self.target = target;
        Ok(())
    }
    /// Consume the display identity and recover only its exclusive allocation for worker reuse.
    pub fn into_buffer(self) -> ProjectionBuffer {
        self.buffer
    }
    /// Distinct application-assigned display generation; it never changes canonical backing.
    pub const fn target(&self) -> FrameStamp {
        self.target
    }
    /// Actual primary canonical generation sampled by this immutable result.
    pub const fn source(&self) -> FrameStamp {
        self.source
    }
    /// Canonical fallback generation sampled only outside primary retained coverage.
    pub const fn underlay(&self) -> Option<FrameStamp> {
        self.underlay
    }
    /// Actual source and approximate output cameras represented by these pixels.
    pub const fn view(&self) -> ViewProjection {
        self.view
    }
    /// Opaque top-row-first sRGB `0xffRRGGBB` words with output-viewport dimensions.
    pub fn pixels(&self) -> &[u32] {
        &self.buffer.pixels
    }
    /// Writable display words for presentation-only marks drawn over the projection.
    pub(crate) fn pixels_mut(&mut self) -> &mut [u32] {
        &mut self.buffer.pixels
    }
}

#[cfg(test)]
mod tests;
