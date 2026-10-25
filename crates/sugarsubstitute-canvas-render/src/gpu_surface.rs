// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish immutable GPU viewport generations after exact native retained-pixel repairs complete.
use crate::{PreparedGpuUpdate, RenderError, SurfaceWork};
use std::sync::Arc;
use sugarsubstitute_canvas::FrameStamp;
use sugarsubstitute_contracts::execution::{JobContext, SurfaceId};
use sugarsubstitute_image_gpu::{DisplayCopy, DisplayImage, GpuDevice};
use sugarsubstitute_image_ops::{Extent, ImageError};

/// One completed opaque sRGB8 GPU viewport with its actual application-assigned identity.
/// Clones share immutable pixels and their original admission through presentation or disposal.
#[derive(Clone)]
pub struct GpuFrame {
    target: FrameStamp,
    image: DisplayImage,
}
impl GpuFrame {
    /// Exact surface/generation represented by every pixel of this immutable image.
    pub const fn target(&self) -> FrameStamp {
        self.target
    }
    /// Completed device pixels for native presentation, qualification or export.
    pub const fn image(&self) -> &DisplayImage {
        &self.image
    }
}
/// Worker-owned retained GPU viewport. Native peers consume cloned immutable completed frames.
/// Construction creates no window or scheduler. Replacements preserve preceding published pixels
/// while native copies execute, and adopt the new generation only after actual GPU completion.
/// Clones have independent generation metadata and share only immutable completed pixels. Prepare
/// on a clone, then adopt it after application document/source revision checks accept the result.
#[derive(Clone)]
pub struct GpuSurface {
    device: Arc<GpuDevice>,
    extent: Extent,
    identity: SurfaceId,
    current: Option<GpuFrame>,
}
impl GpuSurface {
    /// Declare a physical viewport on the startup-selected device; allocation happens at first apply.
    /// # Errors
    /// Rejects device extents above the same 16384-pixel edge bound as CPU retained surfaces.
    pub fn new(
        identity: SurfaceId,
        extent: Extent,
        device: Arc<GpuDevice>,
    ) -> Result<Self, RenderError> {
        if extent.width() > 16384 || extent.height() > 16384 {
            return Err(RenderError::Invalid("Canvas device extent exceeds surface bounds"));
        }
        Ok(Self { device, extent, identity, current: None })
    }
    /// Physical dimensions of every prepared update accepted by this owner.
    pub const fn extent(&self) -> Extent {
        self.extent
    }
    /// Last actually completed generation, distinct from requested or merely prepared work.
    pub fn current(&self) -> Option<FrameStamp> {
        self.current.as_ref().map(GpuFrame::target)
    }
    /// Borrow the immutable completed viewport; no pending native writes can change it.
    pub const fn frame(&self) -> Option<&GpuFrame> {
        self.current.as_ref()
    }
    /// Copy valid retained pixels and apply completed GPU display regions on an admitted worker.
    /// The same dependency validator used by CPU presentation rejects stale/wrong-surface updates
    /// before native effects. Cancellation or driver failure leaves the previous generation intact.
    /// # Errors
    /// Preserves original control/admission/native errors and rejects invalid update dependencies.
    pub(crate) fn apply(
        &mut self,
        update: &PreparedGpuUpdate,
        context: &JobContext,
    ) -> Result<SurfaceWork, RenderError> {
        update.update.validate(self.identity, self.extent, self.current())?;
        if context.cancellation_requested() {
            return Err(ImageError::Cancelled.into());
        }
        if context.deadline().is_some_and(|deadline| context.now() >= deadline) {
            return Err(ImageError::Deadline.into());
        }
        let damage = update.plan();
        if damage.blit().is_none()
            && damage.dirty().is_empty()
            && let Some(previous) = &self.current
        {
            self.current =
                Some(GpuFrame { target: update.target(), image: previous.image.clone() });
            return Ok(SurfaceWork::default());
        }
        let full = damage.dirty().regions().any(|region| region == self.extent.region());
        let previous = if full { None } else { self.current.as_ref().map(GpuFrame::image) };
        let translation = damage
            .blit()
            .map(|blit| DisplayCopy { source: blit.source, destination: blit.destination });
        let image = DisplayImage::compose(
            self.device.clone(),
            self.extent,
            previous,
            translation,
            update.tiles(),
            context,
        )?;
        self.current = Some(GpuFrame { target: update.target(), image });
        Ok(SurfaceWork {
            reused_pixels: damage.blit().map_or(0, |blit| blit.source.pixels()),
            repaired_pixels: damage.dirty().pixels(),
        })
    }
}
