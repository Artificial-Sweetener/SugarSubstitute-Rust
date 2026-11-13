// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Opaque overlay pixels, such as selection outlines, drawn as points over a native display.
//!
//! Pixels travel in storage buffers bounded like every other operation workspace; an overlay of
//! any length is split across as many admitted buffers as it needs, all drawn in the display's
//! render pass in order, so later pixels cover earlier ones.
use crate::{GpuDevice, GpuError, buffer::Buffer};
use std::{borrow::Cow, sync::Arc};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::Extent;

/// Largest storage buffer one batch uses, matching the bounded operation workspace.
const BATCH_BYTES: usize = 4 * 1024 * 1024;
/// Header words before a batch's pixels: output width and height.
const HEADER: usize = 2;
/// Pixels one batch carries, two words each.
const BATCH_PIXELS: usize = BATCH_BYTES / 8 - HEADER;

/// One opaque device pixel drawn over a display, such as a dash of selection outline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverlayPixel {
    /// Output column.
    pub x: u32,
    /// Output row.
    pub y: u32,
    /// Encoded sRGB colour.
    pub color: [u8; 3],
}

/// Uploaded overlay batches and their point counts, retained through the draw's retirement.
pub(crate) struct OverlayBatches {
    batches: Vec<(Arc<Buffer>, u32)>,
}
impl OverlayBatches {
    /// Encode and upload `overlay` for an output of `extent`, admitting each batch's host and
    /// device storage before any native output is acquired.
    pub(crate) fn upload(
        device: &Arc<GpuDevice>,
        overlay: &[OverlayPixel],
        output: Extent,
        context: &JobContext,
    ) -> Result<Self, GpuError> {
        if overlay.iter().any(|pixel| pixel.x >= output.width() || pixel.y >= output.height()) {
            return Err(GpuError::invalid("Display overlay pixels exceed their output"));
        }
        let mut batches = Vec::new();
        batches
            .try_reserve_exact(overlay.len().div_ceil(BATCH_PIXELS))
            .map_err(|cause| GpuError::new("gpu_display_overlay_allocation", cause))?;
        for pixels in overlay.chunks(BATCH_PIXELS) {
            crate::adapter::check(context)?;
            let length = (HEADER + 2 * pixels.len()) * 4;
            let _staging = context
                .memory()
                .claim(length as u64 + 128)
                .map_err(|cause| GpuError::new("gpu_display_overlay_admission", cause))?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(length)
                .map_err(|cause| GpuError::new("gpu_display_overlay_allocation", cause))?;
            for word in [output.width(), output.height()] {
                bytes.extend_from_slice(&word.to_le_bytes());
            }
            for pixel in pixels {
                let [red, green, blue] = pixel.color;
                for word in [pixel.x | pixel.y << 16, u32::from_be_bytes([0, red, green, blue])] {
                    bytes.extend_from_slice(&word.to_le_bytes());
                }
            }
            let buffer = device.allocate(
                length as u64,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                context,
            )?;
            device.write_buffer(&buffer, &bytes);
            batches.push((buffer, pixels.len() as u32));
        }
        Ok(Self { batches })
    }
    /// Draw every batch in order with the overlay pipeline inside an open display pass.
    pub(crate) fn draw(
        &self,
        device: &GpuDevice,
        pipeline: &wgpu::RenderPipeline,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        for (buffer, points) in &self.batches {
            let group = device.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("SugarSubstitute native display overlay"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.raw.as_entire_binding(),
                }],
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..*points, 0..1);
        }
    }
}

/// The point pipeline drawing overlay pixels over the display in the same pass.
pub(crate) fn pipeline(device: &GpuDevice, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
    let module = device.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("SugarSubstitute native display overlay shader"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SOURCE)),
    });
    device.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("SugarSubstitute native display overlay pipeline"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::PointList,
            ..Default::default()
        },
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

const SOURCE: &str = r#"// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
@group(0) @binding(0) var<storage, read> batch: array<u32>;
struct Point {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) color: u32,
}
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> Point {
    let entry = 2u + index * 2u;
    let packed = batch[entry];
    let size = vec2<f32>(f32(batch[0]), f32(batch[1]));
    let centre = vec2<f32>(f32(packed & 65535u), f32(packed >> 16u)) + vec2<f32>(0.5);
    let clip = vec2<f32>(centre.x / size.x * 2.0 - 1.0, 1.0 - centre.y / size.y * 2.0);
    return Point(vec4<f32>(clip, 0.0, 1.0), batch[entry + 1u]);
}
@fragment
fn fragment(point: Point) -> @location(0) vec4<f32> {
    let color = point.color;
    return vec4<f32>(f32((color >> 16u) & 255u), f32((color >> 8u) & 255u),
        f32(color & 255u), 255.0) / 255.0;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlay_shader_requires_no_optional_device_capabilities() -> Result<(), GpuError> {
        let module = wgpu::naga::front::wgsl::parse_str(SOURCE)
            .map_err(|cause| GpuError::new("window_overlay_shader_parse", cause))?;
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .map_err(|cause| GpuError::new("window_overlay_shader_validate", cause))?;
        assert_eq!((BATCH_PIXELS + HEADER) * 8, BATCH_BYTES);
        Ok(())
    }
}
