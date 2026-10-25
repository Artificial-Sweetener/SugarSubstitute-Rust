// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compile shared image semantics and retain every dispatch resource through native retirement.
use crate::{
    GpuDevice, GpuError, GpuTile,
    buffer::Buffer,
    transfer::{Mapping, Transfer},
};
use std::{borrow::Cow, sync::Arc};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::{Pixel, Region};

pub(crate) struct Kernels {
    pipeline: wgpu::ComputePipeline,
    device: Arc<GpuDevice>,
    _memory: MemoryClaim,
}
pub(crate) struct Parameters(pub(crate) [u32; 32]);
impl Parameters {
    pub(crate) fn new(mode: u32, output: Region) -> Self {
        let mut words = [0; 32];
        words[0] = mode;
        words[16..20].copy_from_slice(&[output.x(), output.y(), output.width(), output.height()]);
        Self(words)
    }
    pub(crate) fn color(&mut self, color: Pixel) {
        self.0[20..24].copy_from_slice(&color.channels().map(f32::to_bits));
    }
    pub(crate) fn scalar(&mut self, value: f32) {
        self.0[28] = value.to_bits();
    }
}
/// Computed tile and alpha summary from the same GPU dispatch; masks can skip unreachable inputs.
pub(crate) struct Computed {
    pub(crate) tile: GpuTile,
    pub(crate) has_alpha: bool,
}
impl Kernels {
    pub(crate) fn new(device: Arc<GpuDevice>, context: &JobContext) -> Result<Self, GpuError> {
        crate::adapter::check(context)?;
        let memory = device
            .memory
            .claim(4 * 1024 * 1024)
            .map_err(|cause| GpuError::new("gpu_kernel_admission", cause))?;
        let module = device.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SugarSubstitute image kernels"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SOURCE)),
        });
        device.faults.check()?;
        let pipeline = device.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SugarSubstitute image operations"),
            layout: None,
            module: &module,
            entry_point: Some("image"),
            compilation_options: Default::default(),
            cache: None,
        });
        device.faults.check()?;
        Ok(Self { pipeline, device, _memory: memory })
    }
    pub(crate) fn dispatch(
        &self,
        mut parameters: Parameters,
        region: Region,
        inputs: [Option<&GpuTile>; 3],
        coefficients: &[u32],
        context: &JobContext,
    ) -> Result<Computed, GpuError> {
        crate::adapter::check(context)?;
        if region.width() > 272 || region.height() > 272 || coefficients.len() > 4096 {
            return Err(GpuError::invalid("GPU kernel exceeds bounded image workspace"));
        }
        let bookkeeping = context
            .memory()
            .claim(32 * 1024)
            .map_err(|cause| GpuError::new("gpu_dispatch_bookkeeping", cause))?;
        let storage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;
        let output = self.device.allocate(region.pixels() * 16, storage, context)?;
        let dummy = self.device.allocate(16, storage, context)?;
        let mut buffers: [Option<Arc<Buffer>>; 8] = std::array::from_fn(|_| None);
        for (index, input) in inputs.iter().enumerate() {
            if let Some(input) = input {
                if !Arc::ptr_eq(&input.buffer.device, &self.device) {
                    return Err(GpuError::invalid("Image inputs belong to different GPU devices"));
                }
                let r = input.region();
                parameters.0[4 + index * 4..8 + index * 4].copy_from_slice(&[
                    r.x(),
                    r.y(),
                    r.width(),
                    r.height(),
                ]);
                buffers[index + 1] = Some(input.buffer.clone());
            } else {
                buffers[index + 1] = Some(dummy.clone());
            }
        }
        let uniform = self.upload_words(&parameters.0, wgpu::BufferUsages::UNIFORM, context)?;
        let taps = self.upload_words(
            if coefficients.is_empty() { &[0, 0] } else { coefficients },
            wgpu::BufferUsages::STORAGE,
            context,
        )?;
        let status = self.device.allocate(4, storage, context)?;
        let receiving = self.device.allocate(
            4,
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            context,
        )?;
        buffers[0] = Some(uniform);
        buffers[4] = Some(output.clone());
        buffers[5] = Some(taps);
        buffers[6] = Some(status.clone());
        buffers[7] = Some(receiving.clone());
        let entries: Vec<_> = buffers[..7]
            .iter()
            .enumerate()
            .filter_map(|(binding, buffer)| {
                buffer.as_ref().map(|buffer| wgpu::BindGroupEntry {
                    binding: binding as u32,
                    resource: buffer.raw.as_entire_binding(),
                })
            })
            .collect();
        let group = self.device.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SugarSubstitute image operation inputs"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        let mut commands =
            self.device.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SugarSubstitute image operation"),
            });
        {
            let mut pass = commands.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(region.width().div_ceil(8), region.height().div_ceil(8), 1);
        }
        commands.copy_buffer_to_buffer(&status.raw, 0, &receiving.raw, 0, 4);
        let mut submitted = Transfer::commands(self.device.clone(), buffers, bookkeeping, commands);
        submitted.retire()?;
        let mut mapping = Mapping::new(receiving);
        mapping.wait()?;
        let flags = {
            let view = mapping.view()?;
            u32::from_le_bytes([view[0], view[1], view[2], view[3]])
        };
        mapping.finish()?;
        crate::adapter::check(context)?;
        if flags & 1 != 0 {
            return Err(GpuError::invalid("Image GPU operation produced invalid canonical pixels"));
        }
        Ok(Computed { tile: GpuTile { buffer: output, region }, has_alpha: flags & 2 != 0 })
    }
    fn upload_words(
        &self,
        words: &[u32],
        usage: wgpu::BufferUsages,
        context: &JobContext,
    ) -> Result<Arc<Buffer>, GpuError> {
        let bytes = (words.len() * 4) as u64;
        let _encoding = context
            .memory()
            .claim(bytes)
            .map_err(|cause| GpuError::new("gpu_coefficient_encoding", cause))?;
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(bytes as usize)
            .map_err(|cause| GpuError::new("gpu_coefficient_allocation", cause))?;
        for word in words {
            encoded.extend_from_slice(&word.to_le_bytes());
        }
        let buffer = self.device.allocate(bytes, usage | wgpu::BufferUsages::COPY_DST, context)?;
        let staging = context
            .memory()
            .claim(bytes)
            .map_err(|cause| GpuError::new("gpu_coefficient_staging", cause))?;
        Transfer::upload(buffer.clone(), staging, &encoded).retire()?;
        Ok(buffer)
    }
}

const SOURCE: &str = r#"// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
// Canonical linear-premultiplied image kernels; sampling coefficients come from image-ops.
struct Parameters {
    control: vec4<u32>,
    a: vec4<u32>,
    b: vec4<u32>,
    c: vec4<u32>,
    output: vec4<u32>,
    color: vec4<f32>,
    reserved: vec4<f32>,
    scalar: vec4<f32>,
}
@group(0) @binding(0) var<uniform> p: Parameters;
@group(0) @binding(1) var<storage, read> a: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> b: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> c: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read_write> output: array<vec4<f32>>;
@group(0) @binding(5) var<storage, read> taps: array<vec2<u32>>;
@group(0) @binding(6) var<storage, read_write> status: atomic<u32>;

fn reconstructed(value: vec4<f32>) -> vec4<f32> {
    let alpha = clamp(value.a, 0.0, 1.0);
    if alpha == 0.0 { return vec4<f32>(0.0); }
    return vec4<f32>(value.rgb * (alpha / value.a), alpha);
}

@compute @workgroup_size(8, 8)
fn image(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= p.output.z || id.y >= p.output.w { return; }
    let index = id.y * p.output.z + id.x;
    var value = vec4<f32>(0.0);
    switch p.control.x {
        case 0u: { value = p.color; }
        case 1u: {
            let horizontal = bitcast<f32>(taps[id.x].x);
            let vertical = bitcast<f32>(taps[id.y].y);
            value = p.color * (horizontal * vertical);
        }
        case 2u: { value = vec4<f32>(a[index].rgb * p.scalar.x, a[index].a); }
        case 3u: { value = b[index] + a[index] * (1.0 - b[index].a); }
        case 4u: { value = a[index] * b[index].a; }
        case 5u: {
            let coverage = p.scalar.x * c[index].a;
            value = a[index] * (1.0 - coverage) + b[index] * coverage;
        }
        case 6u: {
            let base = id.x * 14u;
            for (var tap = 0u; tap < taps[base].x; tap++) {
                let coefficient = taps[base + 1u + tap];
                value += a[id.y * p.a.z + coefficient.x] * bitcast<f32>(coefficient.y);
            }
        }
        case 7u: {
            let base = id.y * 14u;
            for (var tap = 0u; tap < taps[base].x; tap++) {
                let coefficient = taps[base + 1u + tap];
                value += a[coefficient.x * p.a.z + id.x] * bitcast<f32>(coefficient.y);
            }
            value = reconstructed(value);
        }
        case 8u: {
            value = a[taps[id.y].y * p.a.z + taps[id.x].x];
        }
        default: { atomicOr(&status, 1u); }
    }
    if p.control.x != 6u {
        if !all(abs(value) <= vec4<f32>(bitcast<f32>(0x7f7fffffu)))
            || value.a < 0.0 || value.a > 1.0
            || (value.a == 0.0 && any(value.rgb != vec3<f32>(0.0))) {
            atomicOr(&status, 1u);
        }
        if value.a > 0.0 { atomicOr(&status, 2u); }
    }
    output[index] = value;
}
"#;
