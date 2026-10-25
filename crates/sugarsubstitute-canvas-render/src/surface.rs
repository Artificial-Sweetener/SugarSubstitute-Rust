// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Immutable region updates and overlap-safe retained CPU pixels.
use crate::RenderError;
use sugarsubstitute_canvas::{DamagePlan, FrameStamp};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease, SurfaceId};
use sugarsubstitute_image_ops::{Extent, ImageError, Region};

struct Patch {
    region: Region,
    pixels: Vec<u32>,
    _memory: MemoryClaim,
}

/// Worker-prepared opaque sRGB device pixels, paired with an exact preceding-frame dependency.
/// Fixed metadata and variable pixel payloads retain execution-backed memory claims.
pub struct PreparedUpdate {
    patches: [Option<Patch>; 32],
    target: FrameStamp,
    base: Option<FrameStamp>,
    plan: DamagePlan,
    _memory: MemoryClaim,
}
impl PreparedUpdate {
    /// Prepare only dirty regions on an admitted worker. Pixel words are `0xffRRGGBB`.
    /// The producer evaluates the image DAG and fills the borrowed row-major device region.
    /// No mutable storage or producer callback reaches presentation.
    /// # Errors
    /// Rejects invalid frame dependencies, memory pressure, allocator failure, nonopaque pixels,
    /// and the producer's original image errors. No retained surface is modified by preparation.
    pub fn prepare(
        target: FrameStamp,
        base: Option<FrameStamp>,
        plan: DamagePlan,
        memory: &MemoryLease,
        mut produce: impl FnMut(Region, &mut [u32]) -> Result<(), RenderError>,
    ) -> Result<Self, RenderError> {
        if base.is_some_and(|base| {
            base.surface != target.surface || base.generation >= target.generation
        }) {
            return Err(RenderError::Invalid("Invalid canvas repair dependency"));
        }
        if base.is_none()
            && (plan.blit().is_some()
                || plan.dirty().regions().count() != 1
                || plan.dirty().regions().next() != Some(plan.dirty().surface().region()))
        {
            return Err(RenderError::Invalid("First canvas frame must prepare the full surface"));
        }
        let claim = memory.claim(size_of::<Self>() as u64 + 128).map_err(ImageError::from)?;
        let mut patches = [const { None }; 32];
        for (slot, region) in patches.iter_mut().zip(plan.dirty().regions()) {
            let payload = memory.claim(region.pixels() * 4 + 128).map_err(ImageError::from)?;
            let length = usize::try_from(region.pixels())
                .map_err(|_| RenderError::Invalid("Device patch exceeds address space"))?;
            let mut pixels = Vec::new();
            pixels.try_reserve_exact(length).map_err(ImageError::from)?;
            pixels.resize(length, 0);
            produce(region, &mut pixels)?;
            if pixels.iter().any(|pixel| pixel >> 24 != 255) {
                return Err(RenderError::Invalid(
                    "Canvas presentation requires opaque device pixels",
                ));
            }
            *slot = Some(Patch { region, pixels, _memory: payload });
        }
        Ok(Self { patches, target, base, plan, _memory: claim })
    }
    /// Exact application-assigned destination generation.
    pub const fn target(&self) -> FrameStamp {
        self.target
    }
    /// Exact retained frame required before applying a partial repair, or none for full replacement.
    pub const fn base(&self) -> Option<FrameStamp> {
        self.base
    }
    /// Camera repair and already-prepared damage; no image operations execute in presentation.
    pub const fn plan(&self) -> &DamagePlan {
        &self.plan
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

/// Opaque sRGB CPU backing surface; it performs no I/O, image evaluation or task spawning.
/// A desktop adapter presents these pixels and owns the native peer lifetime separately.
pub struct CpuSurface {
    pixels: Vec<u32>,
    extent: Extent,
    identity: SurfaceId,
    current: Option<FrameStamp>,
    _memory: MemoryClaim,
}
impl CpuSurface {
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
    /// Borrow top-row-first opaque sRGB `0xffRRGGBB` words; the original charge stays retained.
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }
    /// Apply validated immutable work. Stale/wrong-surface updates leave all pixels unchanged.
    /// Source overlap is handled in both axes; only supplied dirty rectangles receive new pixels.
    /// # Errors
    /// Rejects wrong dimensions/identity, superseded generations and unavailable repair bases.
    pub fn apply(&mut self, update: &PreparedUpdate) -> Result<SurfaceWork, RenderError> {
        if update.target.surface != self.identity || update.plan.dirty().surface() != self.extent {
            return Err(RenderError::Invalid("Canvas update targets a different surface"));
        }
        if self.current.is_some_and(|current| current.generation >= update.target.generation) {
            return Err(RenderError::StaleTarget);
        }
        if update.base.is_some() && update.base != self.current {
            return Err(RenderError::StaleBase);
        }
        let mut work = SurfaceWork::default();
        let stride = self.extent.width() as usize;
        if let Some(blit) = update.plan.blit() {
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
        self.current = Some(update.target);
        Ok(work)
    }
    fn copy_row(&mut self, source: Region, destination: [u32; 2], row: u32, stride: usize) {
        let start = (source.y() + row) as usize * stride + source.x() as usize;
        let target = (destination[1] + row) as usize * stride + destination[0] as usize;
        self.pixels.copy_within(start..start + source.width() as usize, target);
    }
}
