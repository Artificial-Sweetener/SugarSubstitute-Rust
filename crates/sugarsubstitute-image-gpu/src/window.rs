// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native swapchain custody: attach on the window thread, prepare on workers, present completed work.
use crate::{
    DisplayImage, DisplayMapping, DisplayReadback, GpuDevice, GpuError,
    display_mapping::MappingUpload, transfer::Transfer, window_shader::Blitter,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::{Extent, Region};

/// Native handle producer accepted by the safe graphics adapter. Desktop owns window construction.
pub use wgpu::DisplayAndWindowHandle as NativeWindowHandle;

#[derive(Default)]
struct SwapchainClaims {
    current: Option<MemoryClaim>,
    pending: Option<MemoryClaim>,
}
struct SurfaceOwner {
    surface: wgpu::Surface<'static>,
    device: Arc<GpuDevice>,
    format: wgpu::TextureFormat,
    capture: bool,
    follow_up_after_configuration: AtomicBool,
    claims: Mutex<SwapchainClaims>,
    _memory: MemoryClaim,
}
/// Worker-owned native GPU presentation state for one explicitly selected device and window.
/// Attach on the OS window thread; acquire/configure/draw on an admitted worker. Preparation is
/// serialized, and at most one unconsumed completed window frame may remain outstanding.
pub struct WindowSurface {
    blitter: Option<Blitter>,
    owner: Arc<SurfaceOwner>,
    extent: Option<Extent>,
}
/// A completed native swapchain image. The desktop presents it on the original window thread.
/// Dropping an unpublished value discards its already-retired image; original swapchain/window
/// custody remains alive through this token. Dispose rejected values through admitted retirement.
pub struct WindowFrame {
    texture: Option<wgpu::SurfaceTexture>,
    owner: Arc<SurfaceOwner>,
}
/// Native queue acceptance and the bounded recovery work required after configuration.
/// Queue acceptance does not prove compositor delivery or physical display scanout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub struct WindowPresentation {
    /// Present one coalesced follow-up frame so acquisition can observe an opaque surface loss.
    pub needs_follow_up: bool,
}

fn require_follow_up_after_configuration(follow_up: &AtomicBool) {
    follow_up.store(true, Ordering::Release);
}

fn accept_presentation(follow_up: &AtomicBool) -> WindowPresentation {
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
                _memory: memory,
            }),
            extent: None,
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
        self.draw(image, None, context)
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
        self.draw(image, Some(mapping), context)
    }
    /// Configure, acquire and retire one draw while retaining upload and swapchain custody.
    fn draw(
        &mut self,
        image: &DisplayImage,
        mapping: Option<DisplayMapping<'_>>,
        context: &JobContext,
    ) -> Result<Option<WindowFrame>, GpuError> {
        crate::adapter::check(context)?;
        if !Arc::ptr_eq(image.device(), &self.owner.device) {
            return Err(GpuError::invalid(
                "Native window and image belong to different GPU devices",
            ));
        }
        if Arc::strong_count(&self.owner) != 1 {
            return Ok(None);
        }
        let device = &self.owner.device;
        device.faults.check()?;
        let extent = mapping.map_or(image.extent(), DisplayMapping::output);
        if self.extent != Some(extent) {
            let bytes = (u64::from(crate::display::row_bytes(extent.width()))
                * u64::from(extent.height())
                * 3)
            .next_multiple_of(65536)
                + 65536;
            let memory = device
                .memory
                .claim(bytes)
                .map_err(|cause| GpuError::new("gpu_swapchain_admission", cause))?;
            {
                let mut claims =
                    self.owner.claims.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                if claims.pending.is_some() {
                    return Err(GpuError::invalid("Native swapchain configuration did not retire"));
                }
                claims.pending = Some(memory);
            }
            self.owner.surface.configure(
                &device.device,
                &wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | if self.owner.capture {
                            wgpu::TextureUsages::COPY_SRC
                        } else {
                            wgpu::TextureUsages::empty()
                        },
                    format: self.owner.format,
                    color_space: wgpu::SurfaceColorSpace::Srgb,
                    width: extent.width(),
                    height: extent.height(),
                    desired_maximum_frame_latency: 1,
                    present_mode: wgpu::PresentMode::Fifo,
                    alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                    view_formats: vec![],
                },
            );
            {
                let mut claims =
                    self.owner.claims.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                claims.current = claims.pending.take();
            }
            self.extent = Some(extent);
            device.faults.check()?;
            require_follow_up_after_configuration(&self.owner.follow_up_after_configuration);
        }
        if self.blitter.is_none() {
            self.blitter = Some(Blitter::new(device, self.owner.format, context)?);
        }
        let texture = match self.owner.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                self.extent = None;
                texture
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.extent = None;
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                device.faults.check()?;
                return Err(GpuError::invalid(
                    "Native GPU swapchain acquisition failed validation",
                ));
            }
        };
        let mut commands = device.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SugarSubstitute native window frame"),
        });
        let input = image.view();
        let output = texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let blitter = self
            .blitter
            .as_ref()
            .ok_or_else(|| GpuError::invalid("Native window shader is unavailable"))?;
        let upload = MappingUpload::new(mapping, extent, context)?;
        let staging = context
            .memory()
            .claim(upload.bytes.len() as u64 + 128)
            .map_err(|cause| GpuError::new("gpu_display_mapping_staging", cause))?;
        blitter.encode(device, &input, &output, &mut commands);
        device.faults.check()?;
        Transfer::upload(blitter.mapping(), staging, &upload.bytes, Some(commands)).retire()?;
        crate::adapter::check(context)?;
        Ok(Some(WindowFrame { texture: Some(texture), owner: self.owner.clone() }))
    }
}
impl WindowFrame {
    /// Read a bounded region of the actual drawn swapchain image for native qualification.
    /// Call on an admitted worker before presenting; normal presentation performs no readback.
    /// # Errors
    /// Rejects unsupported swapchain capture and invalid regions; preserves original native causes.
    pub fn read_region(
        &self,
        region: Region,
        context: &JobContext,
    ) -> Result<DisplayReadback, GpuError> {
        if !self.owner.capture {
            return Err(GpuError::invalid(
                "Native swapchain does not support qualification readback",
            ));
        }
        let texture = self
            .texture
            .as_ref()
            .ok_or_else(|| GpuError::invalid("Native window frame was already consumed"))?;
        let extent = Extent::new(texture.texture.width(), texture.texture.height())
            .map_err(crate::evaluate::image_error)?;
        let mut readback = crate::display_image::read_texture_region(
            &texture.texture,
            &self.owner.device,
            extent,
            region,
            context,
        )?;
        if self.owner.format == wgpu::TextureFormat::Rgba8Unorm {
            readback.swap_red_blue();
        }
        Ok(readback)
    }
    /// Present already-completed native GPU work on the original desktop window thread.
    /// The returned receipt requests one follow-up acquisition after each configuration because
    /// wgpu does not expose recoverable present status through its public queue API.
    /// # Errors
    /// Preserves original device failures; this operation never waits for an execution job.
    pub fn present(mut self) -> Result<WindowPresentation, GpuError> {
        self.owner.device.faults.check()?;
        let texture = self
            .texture
            .take()
            .ok_or_else(|| GpuError::invalid("Native window frame was already consumed"))?;
        self.owner.device.queue.present(texture);
        self.owner.device.faults.check()?;
        Ok(accept_presentation(&self.owner.follow_up_after_configuration))
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
