// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native swapchain custody: attach on the window thread, prepare and present on admitted workers.
use crate::{
    DisplayImage, DisplayMapping, GpuDevice, GpuError,
    coordination::{ConfigurationIntent, SubmissionPermit},
    surface_state::{Reconfiguration, ReconfigureReason},
    window_shader::{Blitter, StagedDraw},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::Extent;

/// Native handle producer accepted by the safe graphics adapter. Desktop owns window construction.
pub use wgpu::DisplayAndWindowHandle as NativeWindowHandle;

#[derive(Default)]
pub(super) struct SwapchainClaims {
    pub(super) current: Option<MemoryClaim>,
    pub(super) pending: Option<MemoryClaim>,
}
pub(super) struct SurfaceOwner {
    pub(super) surface: wgpu::Surface<'static>,
    pub(super) device: Arc<GpuDevice>,
    pub(super) format: wgpu::TextureFormat,
    pub(super) capture: bool,
    pub(super) follow_up_after_configuration: AtomicBool,
    pub(super) claims: Mutex<SwapchainClaims>,
    pub(super) reconfiguration: Reconfiguration,
    _memory: MemoryClaim,
}
/// Worker-owned native GPU presentation state for one explicitly selected device and window.
/// Attach on the OS window thread; acquire/configure/draw/present on admitted workers. Preparation is
/// serialized, and at most one unconsumed completed window frame may remain outstanding.
pub struct WindowSurface {
    blitter: Option<Blitter>,
    pub(super) owner: Arc<SurfaceOwner>,
    pub(super) extent: Option<Extent>,
    pub(super) pending_configuration: Option<PendingConfiguration>,
}
pub(super) struct PendingConfiguration {
    pub(super) extent: Extent,
    pub(super) reason: ReconfigureReason,
    pub(super) intent: ConfigurationIntent,
}
/// A completed native swapchain image transferred into an admitted native-presentation job.
/// Dropping an unpublished value marks the surface for exclusive reconfiguration before another
/// acquisition; original swapchain/window and submission custody remain alive through this token.
pub struct WindowFrame {
    // Field order makes an unpublished SurfaceTexture drop before its submission lease releases.
    pub(super) texture: Option<wgpu::SurfaceTexture>,
    pub(super) owner: Arc<SurfaceOwner>,
    pub(super) submission: SubmissionPermit,
}
/// Native queue acceptance and the bounded recovery work required after configuration.
/// Queue acceptance does not prove compositor delivery or physical display scanout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub struct WindowPresentation {
    /// Present one coalesced follow-up frame so acquisition can observe an opaque surface loss.
    pub needs_follow_up: bool,
}

pub(super) fn require_follow_up_after_configuration(follow_up: &AtomicBool) {
    follow_up.store(true, Ordering::Release);
}

pub(super) fn accept_presentation(follow_up: &AtomicBool) -> WindowPresentation {
    WindowPresentation { needs_follow_up: follow_up.swap(false, Ordering::AcqRel) }
}

impl WindowSurface {
    /// Bind this selected device to a desktop-owned native peer without creating another device.
    /// Call on the OS window thread, including the macOS main thread. Heavy pipeline preparation
    /// remains deferred to `prepare`; the safe surface retains the supplied window handle producer.
    /// # Errors
    /// Preserves native/admission causes and rejects surfaces without opaque SDR unorm presentation.
    pub fn attach(
        device: Arc<GpuDevice>,
        window: impl NativeWindowHandle + 'static,
    ) -> Result<Self, GpuError> {
        let memory = device
            .memory
            .claim(64 * 1024)
            .map_err(|cause| GpuError::new("gpu_window_metadata", cause))?;
        let surface = device
            .instance
            .create_surface(window)
            .map_err(|cause| GpuError::new("gpu_window_attachment", cause))?;
        let capabilities = surface.get_capabilities(&device.adapter);
        let format = [wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm]
            .into_iter()
            .find(|format| capabilities.formats.contains(format))
            .ok_or_else(|| {
                GpuError::invalid("Selected GPU cannot present SDR pixels to this window")
            })?;
        if !capabilities.present_modes.contains(&wgpu::PresentMode::Fifo)
            || !capabilities.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque)
        {
            return Err(GpuError::invalid("Native GPU window lacks opaque FIFO presentation"));
        }
        let capture = capabilities.usages.contains(wgpu::TextureUsages::COPY_SRC);
        device.faults.check()?;
        Ok(Self {
            owner: Arc::new(SurfaceOwner {
                surface,
                device,
                format,
                capture,
                follow_up_after_configuration: AtomicBool::new(false),
                claims: Mutex::new(SwapchainClaims::default()),
                reconfiguration: Reconfiguration::default(),
                _memory: memory,
            }),
            extent: None,
            pending_configuration: None,
            blitter: None,
        })
    }
    /// Acquire and draw a completed native frame on an admitted worker, observing its actual GPU
    /// completion before returning. This never reads display pixels back to the CPU.
    /// `None` reports temporary occlusion, timeout, reconfiguration or an unconsumed prior token;
    /// the application coalesces a later presentation attempt while retaining its completed image.
    /// # Errors
    /// Rejects foreign devices and preserves original control/admission/driver causes.
    pub fn prepare(
        &mut self,
        image: &DisplayImage,
        context: &JobContext,
    ) -> Result<Option<WindowFrame>, GpuError> {
        self.draw(image, None, None, context)
    }
    /// Project one immutable retained image into a current physical viewport on the same device.
    /// Supplied indices are prevalidated; no source pixels are read back or replaced. Outstanding
    /// frame/backpressure and native completion semantics match canonical presentation.
    /// # Errors
    /// Rejects foreign source extents/devices; preserves original control, admission and GPU causes.
    pub fn prepare_projected(
        &mut self,
        image: &DisplayImage,
        mapping: DisplayMapping<'_>,
        context: &JobContext,
    ) -> Result<Option<WindowFrame>, GpuError> {
        if mapping.source() != image.extent() {
            return Err(GpuError::invalid("Display projection source differs from retained image"));
        }
        self.draw(image, Some(mapping), None, context)
    }
    /// Project a retained detail image over a retained fallback image into one physical viewport.
    /// Covered detail pixels take precedence, uncovered detail pixels sample the underlay, and
    /// pixels uncovered by both mappings use the primary mapping's opaque matte. Both immutable
    /// images and maps are validated before native swapchain acquisition.
    /// # Errors
    /// Rejects foreign source extents/devices or disagreeing output extents; preserves original
    /// control, admission and GPU causes.
    pub fn prepare_composited(
        &mut self,
        image: &DisplayImage,
        mapping: DisplayMapping<'_>,
        underlay: &DisplayImage,
        underlay_mapping: DisplayMapping<'_>,
        context: &JobContext,
    ) -> Result<Option<WindowFrame>, GpuError> {
        if mapping.source() != image.extent() || underlay_mapping.source() != underlay.extent() {
            return Err(GpuError::invalid(
                "Display composition source differs from retained image",
            ));
        }
        if mapping.output() != underlay_mapping.output() {
            return Err(GpuError::invalid(
                "Display composition mappings disagree on output extent",
            ));
        }
        self.draw(image, Some(mapping), Some((underlay, underlay_mapping)), context)
    }
    /// Configure, acquire and retire one draw while retaining upload and swapchain custody.
    fn draw<'primary, 'underlay>(
        &mut self,
        image: &DisplayImage,
        mapping: Option<DisplayMapping<'primary>>,
        underlay: Option<(&DisplayImage, DisplayMapping<'underlay>)>,
        context: &JobContext,
    ) -> Result<Option<WindowFrame>, GpuError> {
        crate::adapter::check(context)?;
        if !Arc::ptr_eq(image.device(), &self.owner.device) {
            return Err(GpuError::invalid(
                "Native window and image belong to different GPU devices",
            ));
        }
        if underlay.is_some_and(|(image, _)| !Arc::ptr_eq(image.device(), &self.owner.device)) {
            return Err(GpuError::invalid(
                "Native window and underlay belong to different GPU devices",
            ));
        }
        if Arc::strong_count(&self.owner) != 1 {
            return Ok(None);
        }
        let device = self.owner.device.clone();
        device.faults.check()?;
        let extent = mapping.map_or(image.extent(), DisplayMapping::output);
        let Some(submission) = self.frame_submission(extent, context)? else {
            return Ok(None);
        };
        if self.blitter.is_none() {
            self.blitter = Some(Blitter::new(&device, self.owner.format, context)?);
        }
        let staged =
            StagedDraw::new(mapping, underlay.map(|(_, mapping)| mapping), extent, context)?;
        let acquired = {
            let _timing = device.timings.begin(crate::timing::Phase::Acquire, 0, 1);
            self.owner.surface.get_current_texture()
        };
        let texture = match acquired {
            wgpu::CurrentSurfaceTexture::Success(texture) => {
                self.owner.reconfiguration.mark(ReconfigureReason::Discarded);
                texture
            }
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                self.owner.reconfiguration.mark(ReconfigureReason::Suboptimal);
                texture
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.owner.reconfiguration.mark(ReconfigureReason::Outdated);
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.owner.reconfiguration.mark(ReconfigureReason::Lost);
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                device.faults.check()?;
                return Err(GpuError::invalid(
                    "Native GPU swapchain acquisition failed validation",
                ));
            }
        };
        let output = texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let blitter = self
            .blitter
            .as_ref()
            .ok_or_else(|| GpuError::invalid("Native window shader is unavailable"))?;
        blitter.submit(
            &device,
            staged,
            image,
            underlay.map(|(image, _)| image),
            &output,
            submission.clone(),
        )?;
        crate::adapter::check(context)?;
        Ok(Some(WindowFrame { texture: Some(texture), owner: self.owner.clone(), submission }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_surface_requests_exactly_one_follow_up_after_acceptance() {
        let follow_up = AtomicBool::new(false);
        assert!(!accept_presentation(&follow_up).needs_follow_up);

        require_follow_up_after_configuration(&follow_up);
        assert!(accept_presentation(&follow_up).needs_follow_up);
        assert!(!accept_presentation(&follow_up).needs_follow_up);

        require_follow_up_after_configuration(&follow_up);
        assert!(accept_presentation(&follow_up).needs_follow_up);
        assert!(!accept_presentation(&follow_up).needs_follow_up);
    }

    #[test]
    fn dropped_candidate_preserves_configured_surface_follow_up() {
        let follow_up = AtomicBool::new(false);
        require_follow_up_after_configuration(&follow_up);

        // Dropping a prepared token never reaches queue acceptance and cannot consume the probe.
        assert!(follow_up.load(Ordering::Acquire));
        require_follow_up_after_configuration(&follow_up);
        assert!(accept_presentation(&follow_up).needs_follow_up);
        assert!(!accept_presentation(&follow_up).needs_follow_up);
    }
}
