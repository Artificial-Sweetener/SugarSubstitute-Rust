// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Immutable region updates and overlap-safe retained CPU pixels.
use crate::{RenderError, update::UpdatePlan};
use sugarsubstitute_canvas::{DamagePlan, FrameStamp};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease, SurfaceId};
use sugarsubstitute_image_ops::{Extent, ImageError, Region, is_premultiplied_word};

#[cfg(test)]
#[path = "surface/adoption_tests.rs"]
mod adoption_tests;
mod preparation;
pub(crate) use preparation::Preparation;

struct Patch {
    region: Region,
    pixels: Vec<u32>,
    _memory: MemoryClaim,
}

/// Worker-prepared premultiplied sRGB device pixels, paired with an exact preceding-frame dependency.
/// Fixed metadata and variable pixel payloads retain execution-backed memory claims.
pub struct PreparedUpdate {
    patches: [Option<Patch>; 32],
    update: UpdatePlan,
    _memory: MemoryClaim,
}
impl PreparedUpdate {
    /// Prepare only dirty regions on an admitted worker. Pixel words are premultiplied
    /// `0xAARRGGBB`; frames composited over an opaque matte are opaque.
    /// The producer evaluates the image DAG and fills the borrowed row-major device region.
    /// No mutable storage or producer callback reaches presentation.
    /// # Errors
    /// Rejects invalid frame dependencies, memory pressure, allocator failure, pixels whose color
    /// exceeds their coverage,
    /// and the producer's original image errors. No retained surface is modified by preparation.
    pub fn prepare(
        target: FrameStamp,
        base: Option<FrameStamp>,
        plan: DamagePlan,
        memory: &MemoryLease,
        produce: impl FnMut(Region, &mut [u32]) -> Result<(), RenderError>,
    ) -> Result<Self, RenderError> {
        Self::prepare_validated(UpdatePlan::new(target, base, plan)?, memory, produce)
    }
    pub(crate) fn prepare_validated(
        update: UpdatePlan,
        memory: &MemoryLease,
        mut produce: impl FnMut(Region, &mut [u32]) -> Result<(), RenderError>,
    ) -> Result<Self, RenderError> {
        let mut prepared = Self::allocate(update, memory)?;
        for patch in prepared.patches.iter_mut().flatten() {
            produce(patch.region, &mut patch.pixels)?;
            if !patch.pixels.iter().all(|pixel| is_premultiplied_word(*pixel)) {
                return Err(RenderError::Invalid(
                    "Canvas presentation requires premultiplied device pixels",
                ));
            }
        }
        Ok(prepared)
    }
    fn allocate(update: UpdatePlan, memory: &MemoryLease) -> Result<Self, RenderError> {
        let claim = memory.claim(size_of::<Self>() as u64 + 128).map_err(ImageError::from)?;
        let mut patches = [const { None }; 32];
        for (slot, region) in patches.iter_mut().zip(update.damage.dirty().regions()) {
            let payload = memory.claim(region.pixels() * 4 + 128).map_err(ImageError::from)?;
            let length = usize::try_from(region.pixels())
                .map_err(|_| RenderError::Invalid("Device patch exceeds address space"))?;
            let mut pixels = Vec::new();
            pixels.try_reserve_exact(length).map_err(ImageError::from)?;
            pixels.resize(length, 0);
            *slot = Some(Patch { region, pixels, _memory: payload });
        }
        Ok(Self { patches, update, _memory: claim })
    }
    /// Exact application-assigned destination generation.
    pub const fn target(&self) -> FrameStamp {
        self.update.target
    }
    /// Exact retained frame required before applying a partial repair, or none for full replacement.
    pub const fn base(&self) -> Option<FrameStamp> {
        self.update.base
    }
    /// Camera repair and already-prepared damage; no image operations execute in presentation.
    pub const fn plan(&self) -> &DamagePlan {
        &self.update.damage
    }
}

/// Actual retained-pixel work, separating image repair from inexpensive memory translation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SurfaceWork {
    /// Device pixels copied from an already presented frame.
    pub reused_pixels: u64,
    /// Newly evaluated device pixels transferred from immutable prepared regions.
    pub repaired_pixels: u64,
}

/// Premultiplied sRGB CPU backing surface; it performs no I/O, image evaluation or task spawning.
/// A desktop adapter presents these pixels and owns the native peer lifetime separately.
pub struct CpuSurface {
    pixels: Vec<u32>,
    extent: Extent,
    identity: SurfaceId,
    current: Option<FrameStamp>,
    _memory: MemoryClaim,
}
impl CpuSurface {
    /// Adopt one complete premultiplied prepared image without allocating a second viewport buffer.
    /// The original pixel claim travels with its storage; partial or dependent updates are rejected.
    /// # Errors
    /// Rejects updates without one complete independent physical image or excessive dimensions.
    pub fn from_prepared(mut update: PreparedUpdate) -> Result<Self, RenderError> {
        let extent = update.update.damage.dirty().surface();
        if update.base().is_some()
            || update.plan().blit().is_some()
            || update.patches.iter().flatten().count() != 1
            || extent.width() > 16384
            || extent.height() > 16384
        {
            return Err(RenderError::Invalid(
                "Surface adoption requires a complete independent image",
            ));
        }
        let patch = update.patches[0]
            .take()
            .ok_or(RenderError::Invalid("Prepared surface has no pixels"))?;
        if patch.region != extent.region() {
            return Err(RenderError::Invalid(
                "Prepared surface does not cover its physical extent",
            ));
        }
        Ok(Self {
            pixels: patch.pixels,
            extent,
            identity: update.target().surface,
            current: Some(update.target()),
            _memory: patch._memory,
        })
    }
    /// Allocate bounded physical device pixels from the application's existing execution admission.
    /// # Errors
    /// Rejects surfaces beyond 16384 pixels per edge, insufficient admission and allocation failure.
    pub fn new(
        identity: SurfaceId,
        extent: Extent,
        memory: &MemoryLease,
    ) -> Result<Self, RenderError> {
        if extent.width() > 16384 || extent.height() > 16384 {
            return Err(RenderError::Invalid("Canvas device extent exceeds surface bounds"));
        }
        let claim = memory.claim(extent.pixels() * 4 + 128).map_err(ImageError::from)?;
        let length = usize::try_from(extent.pixels())
            .map_err(|_| RenderError::Invalid("Canvas surface exceeds address space"))?;
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(length).map_err(ImageError::from)?;
        pixels.resize(length, 0xff000000);
        Ok(Self { pixels, extent, identity, current: None, _memory: claim })
    }
    /// Current actual device-pixel dimensions.
    pub const fn extent(&self) -> Extent {
        self.extent
    }
    /// Most recently applied generation, distinct from requested or merely prepared work.
    pub const fn current(&self) -> Option<FrameStamp> {
        self.current
    }
    /// Borrow top-row-first premultiplied sRGB `0xAARRGGBB` words; the original charge stays retained.
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }
    /// Apply validated immutable work. Stale/wrong-surface updates leave all pixels unchanged.
    /// Source overlap is handled in both axes; only supplied dirty rectangles receive new pixels.
    /// # Errors
    /// Rejects wrong dimensions/identity, superseded generations and unavailable repair bases.
    pub fn apply(&mut self, update: &PreparedUpdate) -> Result<SurfaceWork, RenderError> {
        update.update.validate(self.identity, self.extent, self.current)?;
        let mut work = SurfaceWork::default();
        let stride = self.extent.width() as usize;
        if let Some(blit) = update.update.damage.blit() {
            let source = blit.source;
            if blit.destination[1] > source.y() {
                for row in (0..source.height()).rev() {
                    self.copy_row(source, blit.destination, row, stride);
                }
            } else {
                for row in 0..source.height() {
                    self.copy_row(source, blit.destination, row, stride);
                }
            }
            work.reused_pixels = source.pixels();
        }
        for patch in update.patches.iter().flatten() {
            for row in 0..patch.region.height() as usize {
                let target = (patch.region.y() as usize + row) * stride + patch.region.x() as usize;
                let source = row * patch.region.width() as usize;
                let count = patch.region.width() as usize;
                self.pixels[target..target + count]
                    .copy_from_slice(&patch.pixels[source..source + count]);
            }
            work.repaired_pixels += patch.region.pixels();
        }
        self.current = Some(update.update.target);
        Ok(work)
    }
    fn copy_row(&mut self, source: Region, destination: [u32; 2], row: u32, stride: usize) {
        let start = (source.y() + row) as usize * stride + source.x() as usize;
        let target = (destination[1] + row) as usize * stride + destination[0] as usize;
        self.pixels.copy_within(start..start + source.width() as usize, target);
    }
}
