// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep canonical GPU tile buffers and CPU readback tied to their actual execution charges.
use crate::{
    GpuDevice, GpuError,
    transfer::{Mapping, Transfer},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::{Pixel, Region, TILE_EDGE};

pub(crate) struct Buffer {
    pub(crate) raw: wgpu::Buffer,
    pub(crate) device: Arc<GpuDevice>,
    pub(crate) bytes: u64,
    _memory: MemoryClaim,
}
pub(crate) struct Encoding {
    pub(crate) bytes: Vec<u8>,
    _memory: MemoryClaim,
}
impl Encoding {
    pub(crate) fn pixels(
        region: Region,
        pixels: &[Pixel],
        context: &JobContext,
    ) -> Result<Self, GpuError> {
        if region.width() > TILE_EDGE
            || region.height() > TILE_EDGE
            || region.pixels() != pixels.len() as u64
        {
            return Err(GpuError::invalid("GPU source upload requires one complete bounded tile"));
        }
        let bytes = region.pixels() * 16;
        let memory = context
            .memory()
            .claim(bytes)
            .map_err(|cause| GpuError::new("gpu_upload_workspace", cause))?;
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(bytes as usize)
            .map_err(|cause| GpuError::new("gpu_upload_allocation", cause))?;
        for pixel in pixels {
            for channel in pixel.channels() {
                encoded.extend_from_slice(&channel.to_le_bytes());
            }
        }
        Ok(Self { bytes: encoded, _memory: memory })
    }
}
/// Immutable canonical linear-premultiplied RGBA32F pixels in one selected GPU's memory.
/// Clones share the allocation and its original charge. Raw buffers and queues remain private.
#[derive(Clone)]
pub struct GpuTile {
    pub(crate) buffer: Arc<Buffer>,
    pub(crate) region: Region,
}
/// Canonical CPU pixels copied from a completed GPU submission for qualification or export.
pub struct Readback {
    region: Region,
    pixels: Vec<Pixel>,
    _memory: MemoryClaim,
}
impl Readback {
    /// Global pixel coordinates represented by this row-major array.
    pub const fn region(&self) -> Region {
        self.region
    }
    /// Borrow canonical pixels while their original CPU admission remains retained.
    pub fn pixels(&self) -> &[Pixel] {
        &self.pixels
    }
}
impl GpuDevice {
    pub(crate) fn allocate(
        self: &Arc<Self>,
        bytes: u64,
        usage: wgpu::BufferUsages,
        context: &JobContext,
    ) -> Result<Arc<Buffer>, GpuError> {
        crate::adapter::check(context)?;
        self.faults.check()?;
        if bytes == 0 || bytes > 4 * 1024 * 1024 || !bytes.is_multiple_of(4) {
            return Err(GpuError::invalid("GPU buffer exceeds the bounded operation workspace"));
        }
        let memory = self
            .memory
            .claim(bytes + 256)
            .map_err(|cause| GpuError::new("gpu_buffer_admission", cause))?;
        let raw = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SugarSubstitute image data"),
            size: bytes,
            usage,
            mapped_at_creation: false,
        });
        self.faults.check()?;
        Ok(Arc::new(Buffer { raw, device: self.clone(), bytes, _memory: memory }))
    }
    /// Upload one exact bounded source region on an admitted worker; no CPU tile cache is created.
    /// Observe actual upload retirement before returning. Cancellation cannot retire pending writes.
    /// # Errors
    /// Preserves original control, admission, allocation and native errors; rejects incomplete tiles.
    pub fn upload(
        self: &Arc<Self>,
        region: Region,
        pixels: &[Pixel],
        context: &JobContext,
    ) -> Result<GpuTile, GpuError> {
        let encoded = Encoding::pixels(region, pixels, context)?;
        let bytes = encoded.bytes.len() as u64;
        let buffer = self.allocate(
            bytes,
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            context,
        )?;
        let staging = context
            .memory()
            .claim(bytes)
            .map_err(|cause| GpuError::new("gpu_upload_staging", cause))?;
        let mut transfer = Transfer::upload(buffer.clone(), staging, &encoded.bytes, None);
        transfer.retire()?;
        crate::adapter::check(context)?;
        Ok(GpuTile { buffer, region })
    }
}
impl GpuTile {
    /// Exact global source region represented by this immutable device allocation.
    pub const fn region(&self) -> Region {
        self.region
    }
    /// Read back a completed immutable tile on an admitted worker.
    /// Native copy and mapping retirement complete before resource guards or errors can escape.
    /// # Errors
    /// Preserves control/admission/native causes and rejects invalid canonical pixels.
    pub fn read(&self, context: &JobContext) -> Result<Readback, GpuError> {
        let device = &self.buffer.device;
        let bytes = self.buffer.bytes;
        let memory = context
            .memory()
            .claim(bytes + 256)
            .map_err(|cause| GpuError::new("gpu_readback_admission", cause))?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(self.region.pixels() as usize)
            .map_err(|cause| GpuError::new("gpu_readback_allocation", cause))?;
        let receiving = device.allocate(
            bytes,
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            context,
        )?;
        let mut transfer = Transfer::copy(self.buffer.clone(), receiving.clone());
        transfer.retire()?;
        crate::adapter::check(context)?;
        let mut mapping = Mapping::new(receiving);
        mapping.wait()?;
        crate::adapter::check(context)?;
        {
            let mapped = mapping.view()?;
            for pixel in mapped.chunks_exact(16) {
                let mut channels = [0.0; 4];
                for (value, source) in channels.iter_mut().zip(pixel.chunks_exact(4)) {
                    *value = f32::from_le_bytes([source[0], source[1], source[2], source[3]]);
                }
                pixels.push(
                    Pixel::premultiplied(channels)
                        .map_err(|cause| GpuError::new("gpu_readback_pixel", cause))?,
                );
            }
        }
        mapping.finish()?;
        Ok(Readback { region: self.region, pixels, _memory: memory })
    }
}
