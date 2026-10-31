// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own composition textures that the selected Vulkan device draws into and composition reads.
//!
//! Ordering is CPU-observed on both sides, so neither API needs a keyed mutex or an imported
//! semaphore: a Vulkan draw retires on its worker before the frame reaches the UI thread, and
//! every UI-thread read is followed by a D3D11 fence signal that the next drawing worker awaits.

use super::graphics::{CanvasGraphics, CanvasGraphicsError, checkpoint, missing};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_gpu::SharedTextureHandle;
use sugarsubstitute_image_ops::Extent;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        Graphics::{
            Direct3D11::{
                D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_RESOURCE_MISC_SHARED,
                D3D11_RESOURCE_MISC_SHARED_NTHANDLE, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
                ID3D11Texture2D,
            },
            Dxgi::{
                Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
                DXGI_SHARED_RESOURCE_READ, DXGI_SHARED_RESOURCE_WRITE, IDXGIResource1,
            },
        },
        System::Threading::{CreateEventW, WaitForSingleObject},
    },
    core::{Interface, PCWSTR},
};

/// Longest native wait between cancellation and deadline observations.
const READ_WAIT_MILLISECONDS: u32 = 50;

/// One shared BGRA8 texture on the composition device's adapter.
///
/// Workers import its handle and await prior composition reads before each draw; the UI thread
/// copies a completed frame from it and records the read fence value. The texture, its handle and
/// the funding claim stay alive until this value drops after every user has released it.
pub struct SharedCanvasTexture {
    texture: ID3D11Texture2D,
    handle: usize,
    extent: Extent,
    graphics: Arc<CanvasGraphics>,
    reads: AtomicU64,
    _claim: MemoryClaim,
}

impl SharedCanvasTexture {
    /// Allocate and export one shared texture on an admitted worker.
    /// # Errors
    /// Rejects devices without shared-texture support and oversized extents; preserves admission,
    /// control and native allocation failures.
    pub fn create(
        graphics: &Arc<CanvasGraphics>,
        extent: Extent,
        context: &JobContext,
    ) -> Result<Self, CanvasGraphicsError> {
        checkpoint(context)?;
        if graphics.shared_textures().is_none() {
            return Err(missing("canvas_shared_texture_device"));
        }
        if extent.width() > super::MAX_VIEWPORT_EDGE || extent.height() > super::MAX_VIEWPORT_EDGE {
            return Err(CanvasGraphicsError::new(
                "canvas_shared_texture_extent",
                std::io::Error::from(std::io::ErrorKind::InvalidInput),
            ));
        }
        let claim = graphics
            .memory()
            .claim(extent.pixels() * 4 + 64 * 1024)
            .map_err(|cause| CanvasGraphicsError::new("canvas_shared_texture_budget", cause))?;
        let device = graphics.d3d().ok_or_else(|| missing("canvas_d3d_device"))?;
        let description = D3D11_TEXTURE2D_DESC {
            Width: extent.width(),
            Height: extent.height(),
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: (D3D11_RESOURCE_MISC_SHARED.0 | D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0)
                as u32,
        };
        let mut texture = None;
        // SAFETY: The free-threaded device writes one owned texture to this worker's slot.
        unsafe { device.CreateTexture2D(&description, None, Some(&mut texture)) }
            .map_err(|cause| CanvasGraphicsError::new("create_canvas_shared_texture", cause))?;
        let texture = texture.ok_or_else(|| missing("canvas_shared_texture"))?;
        let resource: IDXGIResource1 = texture
            .cast()
            .map_err(|cause| CanvasGraphicsError::new("query_canvas_shared_resource", cause))?;
        // SAFETY: The retained resource exports one NT handle owned by this value until drop.
        let handle = unsafe {
            resource.CreateSharedHandle(
                None,
                (DXGI_SHARED_RESOURCE_READ | DXGI_SHARED_RESOURCE_WRITE).0,
                PCWSTR::null(),
            )
        }
        .map_err(|cause| CanvasGraphicsError::new("export_canvas_shared_texture", cause))?;
        checkpoint(context)?;
        Ok(Self {
            texture,
            handle: handle.0 as usize,
            extent,
            graphics: graphics.clone(),
            reads: AtomicU64::new(0),
            _claim: claim,
        })
    }

    /// Physical dimensions of this texture.
    pub const fn extent(&self) -> Extent {
        self.extent
    }

    /// Exported handle for import by the Vulkan device with the same adapter LUID.
    /// # Errors
    /// Rejects a graphics device that no longer reports shared-texture support.
    pub fn handle(&self) -> Result<SharedTextureHandle, CanvasGraphicsError> {
        let luid = self.graphics.shared_textures().ok_or_else(|| missing("canvas_shared_luid"))?;
        // SAFETY: `create` exported this open NT handle for a single-mip, single-sample BGRA8
        // render-target texture of exactly this extent on the `luid` adapter, without a keyed
        // mutex; this value closes it only on drop, after every borrowed import completed.
        Ok(unsafe { SharedTextureHandle::from_raw(self.handle, self.extent, luid) })
    }

    /// Wait on a drawing worker until every recorded composition read of this texture completed.
    /// # Errors
    /// Preserves cancellation, deadline, device-removal and native wait failures.
    pub fn await_reads(&self, context: &JobContext) -> Result<(), CanvasGraphicsError> {
        let required = self.reads.load(Ordering::Acquire);
        if required == 0 {
            return Ok(());
        }
        let fence = &self.graphics.read_fence().ok_or_else(|| missing("canvas_read_fence"))?.fence;
        // SAFETY: The worker owns this unnamed auto-reset event until it closes it below.
        let event = unsafe { CreateEventW(None, false, false, PCWSTR::null()) }
            .map_err(|cause| CanvasGraphicsError::new("create_canvas_read_event", cause))?;
        let result = (|| loop {
            checkpoint(context)?;
            // SAFETY: The retained free-threaded fence reports its completed value.
            let completed = unsafe { fence.GetCompletedValue() };
            if completed == u64::MAX {
                return Err(missing("canvas_device_removed"));
            }
            if completed >= required {
                return Ok(());
            }
            // SAFETY: The retained fence signals this live event once `required` completes.
            unsafe { fence.SetEventOnCompletion(required, event) }
                .map_err(|cause| CanvasGraphicsError::new("await_canvas_reads", cause))?;
            // SAFETY: The worker waits on its own live event for a bounded interval.
            match unsafe { WaitForSingleObject(event, READ_WAIT_MILLISECONDS) } {
                WAIT_OBJECT_0 | WAIT_TIMEOUT => {}
                _ => {
                    return Err(CanvasGraphicsError::new(
                        "await_canvas_reads",
                        windows::core::Error::from_thread(),
                    ));
                }
            }
        })();
        // SAFETY: The retained free-threaded fence reports its completed value.
        if unsafe { fence.GetCompletedValue() } < required {
            // A registered completion may still signal this event; closing it could signal a
            // reused handle value instead, so the stopped wait retains it for the process.
            tracing::warn!(
                operation = "await_canvas_reads",
                required,
                "Retaining a read-fence event whose completion remains registered"
            );
            return result;
        }
        // SAFETY: The fence passed `required`, so no registration can signal this event again.
        let closed = unsafe { CloseHandle(event) }
            .map_err(|cause| CanvasGraphicsError::new("close_canvas_read_event", cause));
        result.and(closed)
    }

    /// Whether this texture was exported by exactly this composition device.
    pub(crate) fn owned_by(&self, graphics: &Arc<CanvasGraphics>) -> bool {
        Arc::ptr_eq(&self.graphics, graphics)
    }

    /// Borrow the D3D11 texture for a UI-thread composition read.
    pub(crate) fn texture(&self) -> &ID3D11Texture2D {
        &self.texture
    }

    /// Record the read-fence value that follows this texture's latest queued read.
    pub(crate) fn record_reads(&self, value: u64) {
        self.reads.fetch_max(value, Ordering::AcqRel);
    }
}

impl Drop for SharedCanvasTexture {
    fn drop(&mut self) {
        // SAFETY: This value exclusively owns the exported handle and closes it exactly once.
        if let Err(cause) = unsafe { CloseHandle(HANDLE(self.handle as *mut std::ffi::c_void)) } {
            tracing::error!(operation = "close_canvas_shared_texture", cause = %cause,
                "Canvas shared texture handle could not be closed");
        }
    }
}
