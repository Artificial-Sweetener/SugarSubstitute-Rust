// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Convert typed premultiplied encoded display texels into canonical device raster tiles.
use crate::{DisplayImage, GpuDevice, GpuError, GpuTile, buffer::Buffer, transfer::Transfer};
use std::{borrow::Cow, sync::Arc};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::{Region, SrgbDecoder};

pub(crate) struct DisplayDecoder {
    pipeline: wgpu::ComputePipeline,
    table: Arc<Buffer>,
    _memory: MemoryClaim,
}
impl DisplayDecoder {
    pub(crate) fn new(device: &Arc<GpuDevice>, context: &JobContext) -> Result<Self, GpuError> {
        let memory = device
            .memory
            .claim(4 * 1024 * 1024)
            .map_err(|cause| GpuError::new("gpu_display_decode_pipeline", cause))?;
        let _host = context
            .memory()
            .claim(65536 * 4)
            .map_err(|cause| GpuError::new("gpu_display_decode_table", cause))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(65536 * 4)
            .map_err(|cause| GpuError::new("gpu_display_decode_table_allocation", cause))?;
        for alpha in 0..256_u32 {
            crate::adapter::check(context)?;
            for channel in 0..256_u32 {
                let value = if channel <= alpha {
                    SrgbDecoder::decode((alpha << 24) | (channel << 16))
                        .map_err(crate::evaluate::image_error)?
                        .channels()[0]
                } else {
                    0.0
                };
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let table = device.allocate(
            bytes.len() as u64,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            context,
        )?;
        let staging = context
            .memory()
            .claim(bytes.len() as u64)
            .map_err(|cause| GpuError::new("gpu_display_decode_staging", cause))?;
        Transfer::upload(table.clone(), staging, &bytes, None, device.submission(context)?)
            .retire()?;
        let shader = device.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SugarSubstitute canonical display decoding"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SOURCE)),
        });
        let pipeline = device.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SugarSubstitute display decode"),
            layout: None,
            module: &shader,
            entry_point: Some("decode"),
            compilation_options: Default::default(),
            cache: None,
        });
        device.faults.check()?;
        Ok(Self { pipeline, table, _memory: memory })
    }
    pub(crate) fn decode(
        &self,
        image: &DisplayImage,
        region: Region,
        context: &JobContext,
    ) -> Result<GpuTile, GpuError> {
        crate::adapter::check(context)?;
        if image.extent().width() != region.width() || image.extent().height() != region.height() {
            return Err(GpuError::invalid("Projected raster tile dimensions differ"));
        }
        let device = image.device();
        let output = device.allocate(
            region.pixels() * 16,
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            context,
        )?;
        let view = image.view();
        let group = device.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SugarSubstitute display decode input"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry { binding: 1, resource: self.table.raw.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: output.raw.as_entire_binding() },
            ],
        });
        let _commands = context
            .memory()
            .claim(4096)
            .map_err(|cause| GpuError::new("gpu_display_decode_commands", cause))?;
        let mut commands = device.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SugarSubstitute projected source conversion"),
        });
        {
            let mut pass = commands.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("SugarSubstitute canonical source"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(region.width().div_ceil(8), region.height().div_ceil(8), 1);
        }
        device.faults.check()?;
        Transfer::submit(device.clone(), commands, device.submission(context)?).retire()?;
        crate::adapter::check(context)?;
        Ok(GpuTile { buffer: output, region })
    }
}
const SOURCE: &str = r#"
@group(0) @binding(0) var image:texture_2d<f32>;
@group(0) @binding(1) var<storage,read> table:array<f32>;
@group(0) @binding(2) var<storage,read_write> output:array<vec4<f32>>;
@compute @workgroup_size(8,8)
fn decode(@builtin(global_invocation_id) id:vec3<u32>){
 let size=textureDimensions(image);if id.x>=size.x||id.y>=size.y{return;}
 let encoded=vec4<u32>(floor(textureLoad(image,vec2<i32>(id.xy),0)*255.0+vec4<f32>(0.5)));
 let base=encoded.a*256u;
 output[id.y*size.x+id.x]=vec4<f32>(table[base+encoded.r],table[base+encoded.g],table[base+encoded.b],f32(encoded.a)/255.0);
}
"#;
#[cfg(test)]
mod tests {
    #[test]
    fn canonical_display_decode_shader_validates() -> Result<(), Box<dyn std::error::Error>> {
        let module = wgpu::naga::front::wgsl::parse_str(super::SOURCE)?;
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)?;
        Ok(())
    }
}
