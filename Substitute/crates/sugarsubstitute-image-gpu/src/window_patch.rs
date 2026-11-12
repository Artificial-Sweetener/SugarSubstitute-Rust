// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Replace bounded display rectangles with rounded mixtures of clean and filtered GPU pixels.
use crate::{DisplayImage, GpuDevice, GpuError, buffer::Buffer};
use std::{borrow::Cow, sync::Arc};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::{Extent, Region};

/// Borrow two complete same-device encoded images and exact geometric coverage.
/// Mask bytes are covered subpixels in `[0,16]` for each local row-major pixel; no pixel data
/// from the displayed canvas is read to the CPU. A later patch replaces an earlier overlap.
#[derive(Clone, Copy)]
pub struct DisplayPatch<'a> {
    /// Clean projected pixels before any display patch.
    pub original: &'a DisplayImage,
    /// Shared-DAG filtered pixels of exactly the same dimensions.
    pub filtered: &'a DisplayImage,
    /// Destination top-left in physical viewport pixels.
    pub destination: [u32; 2],
    /// Deterministic four-by-four geometric sample counts.
    pub coverage: &'a [u8],
}
pub(crate) struct StagedPatch {
    original: DisplayImage,
    filtered: DisplayImage,
    mask: Arc<Buffer>,
    _staging: MemoryClaim,
}
impl StagedPatch {
    pub(crate) fn upload(
        device: &Arc<GpuDevice>,
        patch: DisplayPatch<'_>,
        output: Extent,
        context: &JobContext,
    ) -> Result<Self, GpuError> {
        let extent = patch.original.extent();
        let destination = Region::new(
            patch.destination[0],
            patch.destination[1],
            extent.width(),
            extent.height(),
        )
        .map_err(crate::evaluate::image_error)?;
        if patch.filtered.extent() != extent
            || destination.intersection(output.region()) != Some(destination)
            || !Arc::ptr_eq(device, patch.original.device())
            || !Arc::ptr_eq(device, patch.filtered.device())
            || patch.coverage.len() as u64 != extent.pixels()
            || patch.coverage.iter().any(|c| *c > 16)
        {
            return Err(GpuError::invalid("Invalid bounded display patch"));
        }
        let bytes = (8 + patch.coverage.len() as u64) * 4;
        if bytes > 4 * 1024 * 1024 {
            return Err(GpuError::invalid("Display patch mask exceeds its bounded buffer"));
        }
        let _encoded = context
            .memory()
            .claim(bytes)
            .map_err(|cause| GpuError::new("gpu_patch_mask_encoding", cause))?;
        let staging = context
            .memory()
            .claim(bytes)
            .map_err(|cause| GpuError::new("gpu_patch_mask_staging", cause))?;
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(bytes as usize)
            .map_err(|cause| GpuError::new("gpu_patch_mask_allocation", cause))?;
        for word in [
            output.width(),
            output.height(),
            patch.destination[0],
            patch.destination[1],
            extent.width(),
            extent.height(),
            0,
            0,
        ] {
            encoded.extend_from_slice(&word.to_le_bytes());
        }
        for coverage in patch.coverage {
            encoded.extend_from_slice(&u32::from(*coverage).to_le_bytes());
        }
        let mask = device.allocate(
            bytes,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            context,
        )?;
        device.write_buffer(&mask, &encoded);
        Ok(Self {
            original: patch.original.clone(),
            filtered: patch.filtered.clone(),
            mask,
            _staging: staging,
        })
    }
    pub(crate) fn draw(
        &self,
        device: &GpuDevice,
        pipeline: &wgpu::RenderPipeline,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        let original = self.original.view();
        let filtered = self.filtered.view();
        let group = device.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SugarSubstitute rounded display patch"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&original),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&filtered),
                },
                wgpu::BindGroupEntry { binding: 2, resource: self.mask.raw.as_entire_binding() },
            ],
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..6, 0..1);
    }
}
pub(crate) fn pipeline(device: &GpuDevice, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
    let module = device.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("SugarSubstitute rounded patch shader"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SOURCE)),
    });
    device.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SugarSubstitute rounded patch pipeline"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fragment"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
const SOURCE: &str = r#"
@group(0) @binding(0) var original:texture_2d<f32>;
@group(0) @binding(1) var filtered:texture_2d<f32>;
@group(0) @binding(2) var<storage,read> mask:array<u32>;
@vertex fn vertex(@builtin(vertex_index) index:u32)->@builtin(position) vec4<f32>{
 let corners=array<vec2<f32>,6>(vec2<f32>(0.0,0.0),vec2<f32>(1.0,0.0),vec2<f32>(0.0,1.0),vec2<f32>(0.0,1.0),vec2<f32>(1.0,0.0),vec2<f32>(1.0,1.0));
 let point=vec2<f32>(f32(mask[2]),f32(mask[3]))+corners[index]*vec2<f32>(f32(mask[4]),f32(mask[5]));
 return vec4<f32>(point.x/f32(mask[0])*2.0-1.0,1.0-point.y/f32(mask[1])*2.0,0.0,1.0);
}
@fragment fn fragment(@builtin(position) position:vec4<f32>)->@location(0) vec4<f32>{
 let local=vec2<u32>(position.xy)-vec2<u32>(mask[2],mask[3]);
 let count=mask[8u+local.y*mask[4]+local.x];if count==0u{discard;}
 let coverage=f32(count)/16.0;
 let base=round(textureLoad(original,vec2<i32>(local),0)*255.0);
 let blur=round(textureLoad(filtered,vec2<i32>(local),0)*255.0);
 return floor(base*(1.0-coverage)+blur*coverage+vec4<f32>(0.5))/255.0;
}
"#;
#[cfg(test)]
mod tests {
    #[test]
    fn rounded_patch_shader_validates_without_optional_capabilities()
    -> Result<(), Box<dyn std::error::Error>> {
        let module = wgpu::naga::front::wgsl::parse_str(super::SOURCE)?;
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)?;
        Ok(())
    }
}
