// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify retained viewing against complete reconstruction and publish explicit proof artifacts.
use super::{
    options::ProofPaths,
    work::{Resources, Source},
    work_error::WorkError,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use sugarsubstitute_canvas::{DamagePlan, DocumentSnapshot, RenderRequest, Viewport};
use sugarsubstitute_canvas_render::{CpuRenderer, CpuSurface};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{EvaluationStats, Filter, Pixel};
use sugarsubstitute_supervision::{ArtifactPublication, write_artifact};

/// Bounded frame evidence captured only from actual applied presentation generations.
#[derive(Clone, Copy, Serialize)]
pub(super) struct Sample {
    pub(super) phase: u8,
    pub(super) preview: bool,
    pub(super) prepare_microseconds: u128,
    pub(super) computed_pixels: u64,
    pub(super) source_pixels: u64,
    pub(super) repaired_pixels: u64,
    pub(super) reused_pixels: u64,
}
/// Move complete retained pixels into one admitted artifact worker; rejection preserves this input.
pub(super) struct Input {
    pub(super) resources: Resources,
    pub(super) document: DocumentSnapshot,
    pub(super) source: Option<Source>,
    pub(super) view: Viewport,
    pub(super) front: CpuSurface,
    pub(super) samples: [Option<Sample>; 32],
    pub(super) paths: ProofPaths,
    pub(super) adapter: Option<sugarsubstitute_image_gpu::AdapterDescriptor>,
}
#[derive(Serialize)]
struct Metrics<'a> {
    schema_version: u32,
    backend: &'static str,
    adapter: Option<AdapterEvidence<'a>>,
    image_extent: [u32; 2],
    viewport_extent: [u32; 2],
    retained_admission_bytes: u64,
    complete_redraw_verified: bool,
    pixel_sha256: String,
    frames: &'a [Option<Sample>],
}
#[derive(Serialize)]
struct AdapterEvidence<'a> {
    name: &'a str,
    vendor: u32,
    device: u32,
}
/// Reconstruct through the same DAG with no retained-frame repair, compare every display pixel,
/// and stream PNG/JSON on the original admitted storage worker. Originals are never modified.
pub(super) fn publish(input: Input, context: &JobContext) -> Result<(), WorkError> {
    let Input { resources, document, source, view, front, samples, paths, adapter } = input;
    let target =
        front.current().ok_or_else(|| WorkError::invalid("Proof has no presented frame"))?;
    let mut renderer = CpuRenderer::new(super::work::SourcePort(source), 64, &resources.memory)?;
    let update = renderer.prepare(
        RenderRequest {
            document: &document,
            view,
            filter: Filter::Lanczos3,
            matte: Pixel::linear(0.018, 0.018, 0.018, 1.0)?,
            target,
            base: None,
            damage: DamagePlan::full(view.surface()),
        },
        context,
    )?;
    let reference = CpuSurface::from_prepared(update)?;
    if reference.pixels() != front.pixels() {
        return Err(WorkError::invalid(
            "Retained viewport differs from complete DAG reconstruction",
        ));
    }
    drop((reference, renderer));
    let mut hash = Sha256::new();
    for pixel in front.pixels() {
        hash.update(pixel.to_be_bytes());
    }
    let digest = hash.finalize();
    let fingerprint = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    let extent = view.surface();
    let document_extent = document.extent();
    let _workspace = context.memory().claim(16 * 1024 * 1024)?;
    let row_bytes = extent.width() as usize * 3;
    let _row_claim = context.memory().claim(row_bytes as u64)?;
    let mut row = Vec::new();
    row.try_reserve_exact(row_bytes).map_err(sugarsubstitute_image_ops::ImageError::from)?;
    row.resize(row_bytes, 0);
    write_artifact(
        &paths.image,
        true,
        ArtifactPublication::CreateOnly,
        128 * 1024 * 1024,
        context,
        |file| {
            let mut encoder = png::Encoder::new(file, extent.width(), extent.height());
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
            encoder.set_compression(png::Compression::Fast);
            let mut writer = encoder.write_header().map_err(WorkError::new)?;
            {
                let mut stream = writer.stream_writer().map_err(WorkError::new)?;
                for source in front.pixels().chunks_exact(extent.width() as usize) {
                    for (rgb, pixel) in row.chunks_exact_mut(3).zip(source) {
                        let [_, red, green, blue] = pixel.to_be_bytes();
                        rgb.copy_from_slice(&[red, green, blue]);
                    }
                    stream.write_all(&row)?;
                }
                stream.finish().map_err(WorkError::new)?;
            }
            writer.finish().map_err(WorkError::new)
        },
    )
    .map_err(WorkError::new)?;
    let count = samples.iter().take_while(|sample| sample.is_some()).count();
    let metrics = Metrics {
        schema_version: 1,
        backend: adapter.as_ref().map_or("cpu", |adapter| match adapter.backend {
            sugarsubstitute_image_gpu::Backend::Vulkan => "vulkan",
            sugarsubstitute_image_gpu::Backend::Metal => "metal",
        }),
        adapter: adapter.as_ref().map(|adapter| AdapterEvidence {
            name: &adapter.name,
            vendor: adapter.vendor,
            device: adapter.device,
        }),
        image_extent: [document_extent.width(), document_extent.height()],
        viewport_extent: [extent.width(), extent.height()],
        retained_admission_bytes: resources.memory.bytes(),
        complete_redraw_verified: true,
        pixel_sha256: fingerprint,
        frames: &samples[..count],
    };
    write_artifact(
        &paths.metrics,
        true,
        ArtifactPublication::CreateOnly,
        64 * 1024,
        context,
        |file| serde_json::to_writer_pretty(file, &metrics),
    )
    .map_err(WorkError::new)?;
    tracing::info!(
        operation = "canvas_viewer_proof",
        width = extent.width(),
        height = extent.height(),
        frames = count,
        "Retained viewing matches complete reconstruction; proof artifacts published"
    );
    Ok(())
}

/// Difference cumulative source/operation counters without attributing earlier work to a new frame.
pub(super) fn difference(current: EvaluationStats, earlier: EvaluationStats) -> (u64, u64) {
    (
        current.computed_pixels.saturating_sub(earlier.computed_pixels),
        current.source_pixels.saturating_sub(earlier.source_pixels),
    )
}
