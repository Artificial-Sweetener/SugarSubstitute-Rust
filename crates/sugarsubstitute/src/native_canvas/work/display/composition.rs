// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Draw retained GPU backings into a ring of textures shared with the main-window composition.

use super::*;
use sugarsubstitute_canvas_render::{RenderError, backdrop::GpuBackdrop};
use sugarsubstitute_desktop::canvas::{SharedCanvasTexture, SharedTextureFactory};
use sugarsubstitute_image_gpu::{DisplayProjection, SharedSurface, SharedTarget};
use sugarsubstitute_image_ops::Extent;

/// Two textures let the composition keep copying the shown frame while the next one is drawn.
const RING: usize = 2;

/// Immutable retained images and mappings for one composition draw.
pub(super) struct Sources<'a> {
    pub(super) frame: &'a GpuFrame,
    pub(super) samples: Option<&'a ProjectionSamples>,
    pub(super) underlay: Option<(&'a GpuUnderlay, &'a ProjectionSamples)>,
    pub(super) matte: Pixel,
    /// Presentation marks drawn over the projection.
    pub(super) overlay: &'a [OverlayPixel],
    pub(super) backdrops: DisplayBackdrops,
    /// Session-retained metadata admission, independent of the temporary display job.
    pub(super) retained: &'a MemoryLease,
}

/// One exported composition texture and its Vulkan import.
struct Slot {
    texture: Arc<SharedCanvasTexture>,
    target: SharedTarget,
}

/// Worker-owned composition display target for one selected device and composition device.
///
/// A slot is reused only when no session frame retains its texture; each reuse first waits for
/// the composition's recorded reads of that texture. Resizing retires slots of the old extent.
pub(in crate::native_canvas) struct CompositionTarget {
    surface: SharedSurface,
    device: Arc<GpuDevice>,
    textures: SharedTextureFactory,
    slots: Vec<Slot>,
    backdrops: Option<GpuBackdrop>,
}

/// A drawn shared texture whose Vulkan work has completed; the composition may copy it now.
pub(in crate::native_canvas) struct CompositionFrame {
    backdrop_generation: u64,
    texture: Arc<SharedCanvasTexture>,
}

impl CompositionFrame {
    /// Effect material revision drawn into this exact completed shared texture.
    pub(in crate::native_canvas) fn backdrop_generation(&self) -> u64 {
        self.backdrop_generation
    }

    /// Texture the composition copies for this frame.
    pub(in crate::native_canvas) fn texture(&self) -> &SharedCanvasTexture {
        &self.texture
    }
}

impl CompositionTarget {
    /// Pair the selected device with the composition texture allocator on its adapter.
    /// # Errors
    /// Rejects an allocator on another adapter and preserves device admission failures.
    pub(in crate::native_canvas) fn new(
        device: Arc<GpuDevice>,
        textures: SharedTextureFactory,
    ) -> Result<Self, WorkError> {
        if device.descriptor().luid != Some(textures.luid()) {
            return Err(WorkError::invalid("Composition device cannot share this GPU's textures"));
        }
        Ok(Self {
            surface: SharedSurface::new(device.clone())?,
            device,
            textures,
            slots: Vec::with_capacity(RING),
            backdrops: None,
        })
    }

    /// Draw one projection into a free slot and observe its GPU completion.
    /// `None` reports that both slots are still retained by session frames.
    pub(super) fn draw(
        &mut self,
        sources: &Sources<'_>,
        context: &JobContext,
        mut reclaim: impl FnMut() -> Result<CacheReclaim, WorkError>,
    ) -> Result<Option<CompositionFrame>, WorkError> {
        let extent = sources
            .samples
            .map_or(sources.frame.image().extent(), |samples| samples.view().target().surface());
        let Some(index) = self.free_slot(extent, context)? else {
            return Ok(None);
        };
        let slot = &self.slots[index];
        slot.texture.await_reads(context).map_err(WorkError::new)?;
        let primary =
            sources.samples.map(|samples| samples.gpu_mapping(sources.matte)).transpose()?;
        let underlay = sources
            .underlay
            .map(|(underlay, samples)| {
                samples.gpu_mapping(sources.matte).map(|mapping| (underlay.frame.image(), mapping))
            })
            .transpose()?;
        let source = DisplayProjection {
            image: sources.frame.image(),
            mapping: primary,
            underlay,
            overlay: sources.overlay,
        };
        loop {
            let result = (|| -> Result<(), RenderError> {
                if sources.backdrops.plan.regions().is_empty() {
                    self.surface.draw_with_patches(&slot.target, source, &[], context)?;
                    return Ok(());
                }
                if self.backdrops.is_none() {
                    self.backdrops = Some(GpuBackdrop::new(
                        self.device.clone(),
                        sources.retained.clone(),
                        context,
                    )?);
                }
                let patches = self
                    .backdrops
                    .as_mut()
                    .ok_or(RenderError::Invalid("GPU backdrop pipeline is unavailable"))?
                    .prepare(&sources.backdrops.plan, source, context)?;
                let mut layers = Vec::new();
                layers
                    .try_reserve_exact(patches.patches().len())
                    .map_err(sugarsubstitute_image_ops::ImageError::from)?;
                layers.extend(patches.patches().iter().map(|patch| patch.layer()));
                self.surface.draw_with_patches(&slot.target, source, &layers, context)?;
                Ok(())
            })();
            match result {
                Err(RenderError::Gpu(cause)) if cause.is_memory_pressure() => match reclaim()? {
                    CacheReclaim::Progress => {}
                    CacheReclaim::Busy => return Ok(None),
                    CacheReclaim::Exhausted => return Err(cause.into()),
                },
                result => {
                    result?;
                    return Ok(Some(CompositionFrame {
                        texture: slot.texture.clone(),
                        backdrop_generation: sources.backdrops.generation,
                    }));
                }
            }
        }
    }

    /// Find or create an unretained slot of this extent, retiring slots of other extents.
    fn free_slot(
        &mut self,
        extent: Extent,
        context: &JobContext,
    ) -> Result<Option<usize>, WorkError> {
        self.slots
            .retain(|slot| slot.texture.extent() == extent || Arc::strong_count(&slot.texture) > 1);
        if let Some(index) = self.slots.iter().position(|slot| {
            slot.texture.extent() == extent && Arc::strong_count(&slot.texture) == 1
        }) {
            return Ok(Some(index));
        }
        if self.slots.len() >= RING {
            return Ok(None);
        }
        let texture = Arc::new(
            SharedCanvasTexture::create(&self.textures, extent, context).map_err(WorkError::new)?,
        );
        let target = SharedTarget::import(&self.device, &texture.handle(), context)?;
        self.slots.push(Slot { texture, target });
        Ok(Some(self.slots.len() - 1))
    }
}
