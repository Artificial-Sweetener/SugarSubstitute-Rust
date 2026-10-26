// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present already-encoded sRGB display bytes through a native unorm swapchain.
use crate::{GpuDevice, GpuError, buffer::Buffer, display_mapping::MappingUpload};
use std::{borrow::Cow, sync::Arc};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};

pub(crate) struct Blitter {
    pipeline: wgpu::RenderPipeline,
    mapping: Arc<Buffer>,
    _memory: MemoryClaim,
}
impl Blitter {
    pub(crate) fn new(
        device: &Arc<GpuDevice>,
        format: wgpu::TextureFormat,
        context: &JobContext,
    ) -> Result<Self, GpuError> {
        crate::adapter::check(context)?;
        let memory = device
            .memory
            .claim(4 * 1024 * 1024)
            .map_err(|cause| GpuError::new("gpu_window_pipeline_admission", cause))?;
        let mapping = device.allocate(
            MappingUpload::CAPACITY,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            context,
        )?;
        let module = device.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SugarSubstitute native display shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SOURCE)),
        });
        device.faults.check()?;
        let pipeline = device.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SugarSubstitute native display pipeline"),
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
        });
        device.faults.check()?;
        Ok(Self { pipeline, mapping, _memory: memory })
    }
    /// Retain the exact mapping buffer through its write and draw submission.
    pub(crate) fn mapping(&self) -> Arc<Buffer> {
        self.mapping.clone()
    }
    pub(crate) fn encode(
        &self,
        device: &GpuDevice,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        commands: &mut wgpu::CommandEncoder,
    ) {
        let group = device.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SugarSubstitute native display image"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input),
                },
                wgpu::BindGroupEntry { binding: 1, resource: self.mapping.raw.as_entire_binding() },
            ],
        });
        let mut pass = commands.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("SugarSubstitute native display"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
}
const SOURCE: &str = r#"// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var<storage, read> mapping: array<u32>;
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let positions = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    return vec4<f32>(positions[index], 0.0, 1.0);
}
@fragment
fn fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let output = vec2<u32>(position.xy);
    if mapping[0] == 0u {
        return textureLoad(image, vec2<i32>(output), 0);
    }
    let x_entry = 8u + output.x * 3u;
    let y_entry = 8u + (mapping[1] + output.y) * 3u;
    let source_x = mapping[x_entry];
    let source_y = mapping[y_entry];
    if source_x == 0xffffffffu || source_y == 0xffffffffu {
        let matte = mapping[3];
        return vec4<f32>(f32((matte >> 16u) & 255u), f32((matte >> 8u) & 255u),
            f32(matte & 255u), 255.0) / 255.0;
    }
    let x_weight = mapping[x_entry + 2u];
    let y_weight = mapping[y_entry + 2u];
    if x_weight == 0u && y_weight == 0u {
        return textureLoad(image, vec2<i32>(i32(source_x), i32(source_y)), 0);
    }
    let next_x = mapping[x_entry + 1u];
    let next_y = mapping[y_entry + 1u];
    let top_left = encoded_pixel(source_x, source_y);
    let top_right = encoded_pixel(next_x, source_y);
    let bottom_left = encoded_pixel(source_x, next_y);
    let bottom_right = encoded_pixel(next_x, next_y);
    let top = top_left * (256u - x_weight) + top_right * x_weight;
    let bottom = bottom_left * (256u - x_weight) + bottom_right * x_weight;
    let value = (top * (256u - y_weight) + bottom * y_weight + vec4<u32>(32768u)) >> vec4<u32>(16u);
    return vec4<f32>(value) / 255.0;
}
fn encoded_pixel(x: u32, y: u32) -> vec4<u32> {
    return vec4<u32>(round(textureLoad(image, vec2<i32>(i32(x), i32(y)), 0) * 255.0));
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_display_shader_requires_no_optional_device_capabilities() -> Result<(), GpuError> {
        let module = wgpu::naga::front::wgsl::parse_str(SOURCE)
            .map_err(|cause| GpuError::new("window_shader_parse", cause))?;
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .map_err(|cause| GpuError::new("window_shader_validate", cause))?;
        Ok(())
    }
}
