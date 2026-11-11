// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Import composition-owned D3D11 textures into the selected Vulkan device.
//!
//! This is the only Vulkan interop adapter. The composition owner allocates each texture on the
//! physical adapter with this device's LUID and exports an NT handle; Vulkan imports that memory
//! as a color target. Neither API waits on the other: Vulkan draws retire on the CPU before the
//! composition owner reads, and the composition owner proves its reads completed before the next
//! draw into the same texture.
use crate::{AdapterLuid, GpuDevice, GpuError};
use ash::vk;
use std::{ffi::c_void, sync::Arc};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::Extent;
use wgpu::hal::api::Vulkan;
use windows::Win32::Foundation::HANDLE;

/// Read the driver-reported adapter LUID through Vulkan 1.1 device identity properties.
/// Returns `None` for older instances/devices and drivers that report no valid LUID.
pub(crate) fn adapter_luid(adapter: &wgpu::Adapter) -> Option<AdapterLuid> {
    // SAFETY: The guard borrows the live adapter only for read-only property queries.
    let hal = unsafe { adapter.as_hal::<Vulkan>() }?;
    let instance = hal.shared_instance();
    if instance.instance_api_version() < vk::API_VERSION_1_1
        || hal.physical_device_capabilities().properties().api_version < vk::API_VERSION_1_1
    {
        return None;
    }
    let mut identity = vk::PhysicalDeviceIDProperties::default();
    {
        let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut identity);
        // SAFETY: Both instance and device support Vulkan 1.1, whose core entry point fills this
        // exact extension chain; every chained structure outlives the call.
        unsafe {
            instance
                .raw_instance()
                .get_physical_device_properties2(hal.raw_physical_device(), &mut properties);
        }
    }
    (identity.device_luid_valid == vk::TRUE).then(|| AdapterLuid::from_bytes(identity.device_luid))
}

/// NT handle of one composition-owned D3D11 display texture, as transferable plain data.
/// The value never closes the handle; its creator retains handle and texture ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SharedTextureHandle {
    raw: usize,
    extent: Extent,
    luid: AdapterLuid,
}

impl SharedTextureHandle {
    /// Describe an exported D3D11 texture for import by the device with the same LUID.
    ///
    /// # Safety
    /// `raw` must be an NT handle returned by `IDXGIResource1::CreateSharedHandle` for a 2D D3D11
    /// texture of exactly `extent`, one mip level, one array slice, one sample,
    /// `DXGI_FORMAT_B8G8R8A8_UNORM`, render-target binding and no keyed mutex, created on the
    /// adapter identified by `luid`. The handle must stay open while any
    /// [`SharedTarget::import`] call using this value runs.
    ///
    /// Correct pixels additionally require the creator not to access the texture while a
    /// [`crate::SharedSurface`] draw into its imported target runs, and to prove its own GPU reads
    /// completed before the next such draw begins. Violating that ordering tears displayed pixels;
    /// it cannot invalidate host memory.
    pub const unsafe fn from_raw(raw: usize, extent: Extent, luid: AdapterLuid) -> Self {
        Self { raw, extent, luid }
    }
    /// Physical dimensions of the exported texture.
    pub const fn extent(&self) -> Extent {
        self.extent
    }
    /// Adapter that allocated the exported texture.
    pub const fn luid(&self) -> AdapterLuid {
        self.luid
    }
}

/// One imported composition texture as a Vulkan color target on the selected device.
/// Dropping it releases only the Vulkan image and memory binding; the D3D11 owner keeps its texture.
pub struct SharedTarget {
    pub(crate) texture: wgpu::Texture,
    pub(crate) extent: Extent,
    pub(crate) device: Arc<GpuDevice>,
    _memory: MemoryClaim,
}

impl SharedTarget {
    /// Import an exported texture on an admitted worker.
    /// # Errors
    /// Rejects devices without shared-texture support, foreign adapters and oversized textures;
    /// preserves admission, control and native import causes.
    pub fn import(
        device: &Arc<GpuDevice>,
        handle: &SharedTextureHandle,
        context: &JobContext,
    ) -> Result<Self, GpuError> {
        crate::adapter::check(context)?;
        if !device.descriptor.shared_textures {
            return Err(GpuError::invalid("Selected GPU cannot import composition textures"));
        }
        if device.descriptor.luid != Some(handle.luid) {
            return Err(GpuError::invalid("Composition texture belongs to another adapter"));
        }
        let extent = handle.extent;
        if extent.width() > 16384 || extent.height() > 16384 {
            return Err(GpuError::invalid("Composition texture exceeds display bounds"));
        }
        let memory = device
            .memory
            .claim(16 * 1024)
            .map_err(|cause| GpuError::new("gpu_shared_texture_metadata", cause))?;
        device.faults.check()?;
        let size = wgpu::Extent3d {
            width: extent.width(),
            height: extent.height(),
            depth_or_array_layers: 1,
        };
        let label = Some("SugarSubstitute composition display target");
        let native = wgpu::hal::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::wgt::TextureUses::COLOR_TARGET,
            memory_flags: wgpu::hal::MemoryFlags::empty(),
            view_formats: Vec::new(),
        };
        let raw = {
            // SAFETY: The guard borrows the live selected device only for this import.
            let hal = unsafe { device.device.as_hal::<Vulkan>() }
                .ok_or_else(|| GpuError::invalid("Selected GPU is not a Vulkan device"))?;
            // SAFETY: The handle contract proves an open D3D11 NT texture handle with exactly this
            // format, extent and usage on this device's adapter, and the device enabled
            // VK_KHR_external_memory_win32 because it reports shared-texture support.
            unsafe {
                hal.texture_from_d3d11_shared_handle(HANDLE(handle.raw as *mut c_void), &native)
            }
            .map_err(|cause| GpuError::new("gpu_shared_texture_import", cause))?
        };
        // SAFETY: `raw` was created from this device's hal handle with the matching descriptor;
        // its previous contents are discarded, so the first barrier starts from undefined.
        let texture = unsafe {
            device.device.create_texture_from_hal::<Vulkan>(
                raw,
                &wgpu::TextureDescriptor {
                    label,
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Bgra8Unorm,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                },
                wgpu::wgt::TextureUses::UNINITIALIZED,
            )
        };
        device.faults.check()?;
        Ok(Self { texture, extent, device: device.clone(), _memory: memory })
    }
    /// Physical dimensions of this imported target.
    pub const fn extent(&self) -> Extent {
        self.extent
    }
}
