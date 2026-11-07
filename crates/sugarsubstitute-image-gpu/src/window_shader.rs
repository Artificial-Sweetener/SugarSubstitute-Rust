// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present already-encoded premultiplied sRGB display bytes through a native unorm target.
use crate::{
    DisplayImage, DisplayMapping, GpuDevice, GpuError, OverlayPixel,
    buffer::Buffer,
    coordination::SubmissionPermit,
    display_mapping::MappingUpload,
    transfer::Transfer,
    window_overlay::{self, OverlayBatches},
};
use std::{borrow::Cow, sync::Arc};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::Extent;

/// Encoded display mapping, uploaded overlay and their admitted staging, prepared before any
/// native output exists. Staging first lets memory pressure reject a draw without abandoning an
/// acquired output image.
pub(crate) struct StagedDraw {
    upload: MappingUpload,
    overlay: OverlayBatches,
    staging: MemoryClaim,
    patches: Vec<crate::window_patch::StagedPatch>,
}

impl StagedDraw {
    /// Encode mappings and upload overlay pixels for one physical output under the caller's
    /// execution admission.
    pub(crate) fn new(
        device: &Arc<GpuDevice>,
        mapping: Option<DisplayMapping<'_>>,
        underlay: Option<DisplayMapping<'_>>,
        overlay: &[OverlayPixel],
        output: Extent,
        context: &JobContext,
    ) -> Result<Self, GpuError> {
        let upload = MappingUpload::new(mapping, underlay, output, context)?;
        let staging = context
            .memory()
            .claim(upload.bytes.len() as u64 + 128)
            .map_err(|cause| GpuError::new("gpu_display_mapping_staging", cause))?;
        let overlay = OverlayBatches::upload(device, overlay, output, context)?;
        Ok(Self { upload, overlay, staging, patches: Vec::new() })
    }
    pub(crate) fn with_patches(
        mut self,
        device: &Arc<GpuDevice>,
        patches: &[crate::DisplayPatch<'_>],
        output: Extent,
        context: &JobContext,
    ) -> Result<Self, GpuError> {
        if patches.len() > 2 {
            return Err(GpuError::invalid("Too many bounded display patches"));
        }
        self.patches
            .try_reserve_exact(patches.len())
            .map_err(|cause| GpuError::new("gpu_patch_inventory", cause))?;
        for patch in patches {
            self.patches
                .push(crate::window_patch::StagedPatch::upload(device, *patch, output, context)?);
        }
        Ok(self)
    }
}

pub(crate) struct Blitter {
    pipeline: wgpu::RenderPipeline,
    overlay: wgpu::RenderPipeline,
    patch: wgpu::RenderPipeline,
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
        let overlay = window_overlay::pipeline(device, format);
        device.faults.check()?;
        let patch = crate::window_patch::pipeline(device, format);
        device.faults.check()?;
        Ok(Self { pipeline, overlay, patch, mapping, _memory: memory })
    }
    /// Retain the exact mapping buffer through its write and draw submission.
    pub(crate) fn mapping(&self) -> Arc<Buffer> {
        self.mapping.clone()
    }
    /// Draw one staged projection into `output` and observe its native completion.
    /// Every input image, mapping buffer and staging claim stays alive through retirement.
    pub(crate) fn submit(
        &self,
        device: &Arc<GpuDevice>,
        staged: StagedDraw,
        image: &DisplayImage,
        underlay: Option<&DisplayImage>,
        output: &wgpu::TextureView,
        submission: SubmissionPermit,
    ) -> Result<(), GpuError> {
        let mut commands = device.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SugarSubstitute native display frame"),
        });
        let input = image.view();
        let underlay_input = underlay.map_or_else(|| image.view(), DisplayImage::view);
        self.encode(
            device,
            &input,
            &underlay_input,
            output,
            &staged.overlay,
            &staged.patches,
            &mut commands,
        );
        device.faults.check()?;
        Transfer::upload(
            self.mapping(),
            staged.staging,
            &staged.upload.bytes,
            Some(commands),
            submission,
        )
        .retire()
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "Keep one admitted display submission and its borrowed image custody together"
    )]
    pub(crate) fn encode(
        &self,
        device: &GpuDevice,
        input: &wgpu::TextureView,
        underlay: &wgpu::TextureView,
        output: &wgpu::TextureView,
        overlay: &OverlayBatches,
        patches: &[crate::window_patch::StagedPatch],
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
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(underlay),
                },
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
        overlay.draw(device, &self.overlay, &mut pass);
        for patch in patches {
            patch.draw(device, &self.patch, &mut pass);
        }
    }
}

const SOURCE: &str = r#"// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var<storage, read> mapping: array<u32>;
@group(0) @binding(2) var underlay: texture_2d<f32>;
struct MappedSource {
    first: vec2<u32>,
    second: vec2<u32>,
    weight: vec2<u32>,
    covered: u32,
}
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
    let primary = mapped_source(output, 8u);
    if primary.covered != 0u {
        return sample_image(primary);
    }
    let underlay_base = mapping[4];
    if underlay_base != 0u {
        let fallback = mapped_source(output, underlay_base);
        if fallback.covered != 0u {
            return sample_underlay(fallback);
        }
    }
    let matte = mapping[3];
    return vec4<f32>(f32((matte >> 16u) & 255u), f32((matte >> 8u) & 255u),
        f32(matte & 255u), f32(matte >> 24u)) / 255.0;
}
fn mapped_source(output: vec2<u32>, base: u32) -> MappedSource {
    let x_entry = base + output.x * 3u;
    let y_entry = base + (mapping[1] + output.y) * 3u;
    let source_x = mapping[x_entry];
    let source_y = mapping[y_entry];
    if source_x == 0xffffffffu || source_y == 0xffffffffu {
        return MappedSource(vec2<u32>(0u), vec2<u32>(0u), vec2<u32>(0u), 0u);
    }
    return MappedSource(
        vec2<u32>(source_x, source_y),
        vec2<u32>(mapping[x_entry + 1u], mapping[y_entry + 1u]),
        vec2<u32>(mapping[x_entry + 2u], mapping[y_entry + 2u]),
        1u,
    );
}
fn sample_image(source: MappedSource) -> vec4<f32> {
    if source.weight.x == 0u && source.weight.y == 0u {
        return textureLoad(image, vec2<i32>(i32(source.first.x), i32(source.first.y)), 0);
    }
    return interpolate(
        encoded_image_pixel(source.first.x, source.first.y),
        encoded_image_pixel(source.second.x, source.first.y),
        encoded_image_pixel(source.first.x, source.second.y),
        encoded_image_pixel(source.second.x, source.second.y),
        source.weight,
    );
}
fn sample_underlay(source: MappedSource) -> vec4<f32> {
    if source.weight.x == 0u && source.weight.y == 0u {
        return textureLoad(underlay, vec2<i32>(i32(source.first.x), i32(source.first.y)), 0);
    }
    return interpolate(
        encoded_underlay_pixel(source.first.x, source.first.y),
        encoded_underlay_pixel(source.second.x, source.first.y),
        encoded_underlay_pixel(source.first.x, source.second.y),
        encoded_underlay_pixel(source.second.x, source.second.y),
        source.weight,
    );
}
fn interpolate(
    top_left: vec4<u32>,
    top_right: vec4<u32>,
    bottom_left: vec4<u32>,
    bottom_right: vec4<u32>,
    weight: vec2<u32>,
) -> vec4<f32> {
    let top = top_left * (256u - weight.x) + top_right * weight.x;
    let bottom = bottom_left * (256u - weight.x) + bottom_right * weight.x;
    let value = (top * (256u - weight.y) + bottom * weight.y + vec4<u32>(32768u)) >> vec4<u32>(16u);
    return vec4<f32>(value) / 255.0;
}
fn encoded_image_pixel(x: u32, y: u32) -> vec4<u32> {
    return vec4<u32>(round(textureLoad(image, vec2<i32>(i32(x), i32(y)), 0) * 255.0));
}
fn encoded_underlay_pixel(x: u32, y: u32) -> vec4<u32> {
    return vec4<u32>(round(textureLoad(underlay, vec2<i32>(i32(x), i32(y)), 0) * 255.0));
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
