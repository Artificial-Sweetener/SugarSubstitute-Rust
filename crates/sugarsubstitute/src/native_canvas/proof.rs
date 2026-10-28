// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify retained viewing against complete reconstruction and publish explicit proof artifacts.
mod presentation;
use super::{
    work::{Resources, Source},
    work_error::WorkError,
};
use presentation::{PresentationEvidence, PresentationTimingContext};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use sugarsubstitute_canvas::{
    DamagePlan, DocumentSnapshot, RenderRequest, ViewProjection, Viewport,
};
use sugarsubstitute_canvas_render::{CpuRenderer, CpuSurface, GpuFrame};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_io::ArchiveStats;
use sugarsubstitute_image_ops::{EvaluationStats, Pixel, Reconstruction, Region};
use sugarsubstitute_supervision::{ArtifactPublication, write_artifact};

/// Explicit caller-owned outputs for headless viewer qualification.
pub(crate) struct ProofPaths {
    /// Destination for the qualified viewport image.
    pub(crate) image: std::path::PathBuf,
    /// Destination for the matching bounded metrics receipt.
    pub(crate) metrics: std::path::PathBuf,
    /// Optional controlled motion interval for presentation measurements.
    pub(crate) motion_duration: std::time::Duration,
}

/// Bounded frame evidence captured only from actual applied presentation generations.
#[derive(Clone, Copy, Serialize)]
pub(super) struct Sample {
    pub(super) phase: u8,
    pub(super) physical_zoom: f64,
    pub(super) nearest_reconstruction: bool,
    pub(super) preview: bool,
    pub(super) prepare_microseconds: u128,
    pub(super) computed_pixels: u64,
    pub(super) source_pixels: u64,
    pub(super) repaired_pixels: u64,
    pub(super) reused_pixels: u64,
}
/// Move complete retained pixels into one admitted artifact worker; rejection preserves this input.
pub(super) enum ProofBacking {
    /// Immutable retained software pixels and their accepted generation.
    Cpu(std::sync::Arc<CpuSurface>),
    /// Immutable retained device pixels read back only by the final artifact worker.
    Gpu(GpuFrame),
}
impl ProofBacking {
    /// Accepted backing generation represented by the retained pixels.
    pub(super) fn target(&self) -> Option<sugarsubstitute_canvas::FrameStamp> {
        match self {
            Self::Cpu(surface) => surface.current(),
            Self::Gpu(frame) => Some(frame.target()),
        }
    }
    fn extent(&self) -> sugarsubstitute_image_ops::Extent {
        match self {
            Self::Cpu(surface) => surface.extent(),
            Self::Gpu(frame) => frame.image().extent(),
        }
    }
}
pub(super) struct Input {
    pub(super) resources: Resources,
    pub(super) document: DocumentSnapshot,
    pub(super) source: Option<Source>,
    pub(super) view: Viewport,
    pub(super) backing_view: Viewport,
    pub(super) projection: ViewProjection,
    pub(super) reconstruction: Reconstruction,
    pub(super) backing: ProofBacking,
    pub(super) samples: [Option<Sample>; 32],
    pub(super) paths: ProofPaths,
    pub(super) adapter: Option<sugarsubstitute_image_gpu::AdapterDescriptor>,
    pub(super) presentations: sugarsubstitute_canvas_render::PresentationStats,
    pub(super) motion: Option<sugarsubstitute_canvas_render::PresentationStats>,
    pub(super) warmup: Option<sugarsubstitute_canvas_render::PresentationStats>,
    pub(super) canonical_gpu_snapshot_readback: bool,
    pub(super) native_gpu_display_verification_readback: bool,
    pub(super) coverage_acceptances: u64,
    /// Native-accepted displays using complete fallback coverage or a retained underlay.
    pub(super) fallback_projection_acceptances: u64,
    /// Native-accepted displays whose primary pixels come from canonical navigation pyramids.
    pub(super) navigation_projection_acceptances: u64,
    /// Native-accepted presentations that still sampled the small direct-source bootstrap.
    pub(super) bootstrap_projection_acceptances: u64,
    /// Whether the phase-six zoom-out accepted fallback output before settling.
    pub(super) zoom_out_fallback_exercised: bool,
    /// Archive work used by the first completed current-source fallback preview preparation.
    pub(super) first_fallback_preparation_io: Option<ArchiveStats>,
    /// First source intent through native queue acceptance; includes import/load, not scanout.
    pub(super) first_image_queue_latency: Option<std::time::Duration>,
    /// Whether startup selected the proof source through the ordinary lifetime port.
    pub(super) source_selection_port_exercised: bool,
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
    presentation_timing_context: PresentationTimingContext,
    motion_requested_duration_nanoseconds: u64,
    navigation_presentations: PresentationEvidence,
    motion_and_fit_recovery_presentations: Option<PresentationEvidence>,
    initial_settle_presentations: Option<PresentationEvidence>,
    coverage_validated_presentations: u64,
    fallback_projection_acceptances: u64,
    navigation_projection_acceptances: u64,
    bootstrap_projection_acceptances: u64,
    zoom_out_fallback_exercised: bool,
    initial_fallback_preparation_io: Option<SourceIoEvidence>,
    /// First source intent through native queue acceptance; excludes display scanout.
    initial_image_input_to_native_queue_nanoseconds: Option<u64>,
    source_selection_port_exercised: bool,
}
#[derive(Serialize)]
struct AdapterEvidence<'a> {
    name: &'a str,
    vendor: u32,
    device: u32,
}
#[derive(Serialize)]
struct SourceIoEvidence {
    read_bytes: u64,
    chunks: u64,
    range_reads: u64,
    range_seeks: u64,
}
impl From<ArchiveStats> for SourceIoEvidence {
    fn from(stats: ArchiveStats) -> Self {
        Self {
            read_bytes: stats.read_bytes,
            chunks: stats.chunks,
            range_reads: stats.range_reads,
            range_seeks: stats.range_seeks,
        }
    }
}
/// Reconstruct through the same DAG with no retained-frame repair, compare every display pixel,
/// and stream PNG/JSON on the original admitted storage worker. Originals are never modified.
pub(super) fn publish(input: Input, context: &JobContext) -> Result<(), WorkError> {
    tracing::info!(
        operation = "canvas_proof_comparison",
        stage = "reference",
        "Preparing complete reconstruction for retained-frame comparison"
    );
    let Input {
        resources,
        document,
        source,
        view,
        backing_view,
        projection,
        reconstruction,
        backing,
        samples,
        paths,
        adapter,
        presentations,
        motion,
        warmup,
        canonical_gpu_snapshot_readback,
        native_gpu_display_verification_readback,
        coverage_acceptances,
        fallback_projection_acceptances,
        navigation_projection_acceptances,
        bootstrap_projection_acceptances,
        zoom_out_fallback_exercised,
        first_fallback_preparation_io,
        first_image_queue_latency,
        source_selection_port_exercised,
    } = input;
    if !zoom_out_fallback_exercised || fallback_projection_acceptances == 0 {
        return Err(WorkError::invalid(
            "Proof did not accept bounded fallback output during zoom-out",
        ));
    }
    resources.release_graph_tiles()?;
    let target =
        backing.target().ok_or_else(|| WorkError::invalid("Proof has no presented frame"))?;
    let mut renderer = CpuRenderer::new(super::work::SourcePort(source), 64, &resources.memory)?;
    let update = renderer.prepare(
        RenderRequest {
            document: &document,
            view,
            reconstruction,
            matte: Pixel::linear(0.018, 0.018, 0.018, 1.0)?,
            target,
            base: None,
            damage: DamagePlan::full(view.surface()),
        },
        context,
    )?;
    let reference = CpuSurface::from_prepared(update)?;
    if projection.source() != backing_view
        || projection.target() != view
        || backing.extent() != backing_view.surface()
    {
        tracing::error!(
            operation = "canvas_proof_projection_mismatch",
            source_matches = projection.source() == backing_view,
            target_matches = projection.target() == view,
            extent_matches = backing.extent() == backing_view.surface(),
            "Proof projection differs from retained coverage"
        );
        return Err(WorkError::invalid("Proof backing projection differs from retained coverage"));
    }
    let rgba = sugarsubstitute_image_ops::SrgbEncoder::encode_constant(Pixel::linear(
        0.018, 0.018, 0.018, 1.0,
    )?)?;
    let matte = u32::from_be_bytes([255, rgba[0], rgba[1], rgba[2]]);
    tracing::info!(
        operation = "canvas_proof_comparison",
        stage = "pixels",
        width = reference.extent().width(),
        height = reference.extent().height(),
        navigation_acceptances = navigation_projection_acceptances,
        "Complete reconstruction prepared for retained-frame comparison"
    );
    match &backing {
        ProofBacking::Cpu(front) => compare_cpu(&reference, front, projection, matte)?,
        ProofBacking::Gpu(front) => compare_gpu(&reference, front, projection, matte, context)?,
    }
    drop(renderer);
    let mut hash = Sha256::new();
    for pixel in reference.pixels() {
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
                for source in reference.pixels().chunks_exact(extent.width() as usize) {
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
        schema_version: 5,
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
        presentation_timing_context: PresentationTimingContext {
            canonical_gpu_snapshot_readback,
            native_gpu_display_verification_readback,
        },
        motion_requested_duration_nanoseconds: u64::try_from(paths.motion_duration.as_nanos())
            .map_err(WorkError::new)?,
        navigation_presentations: PresentationEvidence::new(presentations),
        motion_and_fit_recovery_presentations: motion.map(PresentationEvidence::new),
        initial_settle_presentations: warmup.map(PresentationEvidence::new),
        coverage_validated_presentations: coverage_acceptances,
        fallback_projection_acceptances,
        navigation_projection_acceptances,
        bootstrap_projection_acceptances,
        zoom_out_fallback_exercised,
        initial_fallback_preparation_io: first_fallback_preparation_io.map(Into::into),
        initial_image_input_to_native_queue_nanoseconds: first_image_queue_latency
            .map(|latency| u64::try_from(latency.as_nanos()))
            .transpose()
            .map_err(WorkError::new)?,
        source_selection_port_exercised,
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

fn compare_cpu(
    reference: &CpuSurface,
    retained: &CpuSurface,
    projection: ViewProjection,
    matte: u32,
) -> Result<(), WorkError> {
    let source_stride = retained.extent().width() as usize;
    let target_width = reference.extent().width() as usize;
    for (index, actual) in reference.pixels().iter().enumerate() {
        let x = u32::try_from(index % target_width).map_err(WorkError::new)?;
        let y = u32::try_from(index / target_width).map_err(WorkError::new)?;
        let expected = match (projection.column(x), projection.row(y)) {
            (Some(column), Some(row)) => {
                retained.pixels()[row as usize * source_stride + column as usize]
            }
            _ => matte,
        };
        if *actual != expected {
            tracing::error!(
                operation = "canvas_proof_pixel_mismatch",
                backend = "cpu",
                x,
                y,
                maximum_channel_difference = maximum_channel_difference(*actual, expected),
                "Retained crop differs from complete reconstruction"
            );
            return Err(WorkError::invalid(
                "Retained CPU backing crop differs from complete DAG reconstruction",
            ));
        }
    }
    Ok(())
}

fn compare_gpu(
    reference: &CpuSurface,
    retained: &GpuFrame,
    projection: ViewProjection,
    matte: u32,
    context: &JobContext,
) -> Result<(), WorkError> {
    let target_stride = reference.extent().width() as usize;
    for target in reference.extent().region().tiles() {
        let columns = mapped_bounds(target.x(), target.right(), |value| projection.column(value));
        let rows = mapped_bounds(target.y(), target.bottom(), |value| projection.row(value));
        let readback = match (columns, rows) {
            (Some((left, right)), Some((top, bottom))) => Some(retained.image().read_region(
                Region::new(left, top, right - left + 1, bottom - top + 1)?,
                context,
            )?),
            _ => None,
        };
        for y in target.y()..target.bottom() {
            for x in target.x()..target.right() {
                let expected = match (projection.column(x), projection.row(y), readback.as_ref()) {
                    (Some(column), Some(row), Some(readback)) => {
                        let region = readback.region();
                        readback.pixels()[(row - region.y()) as usize * region.width() as usize
                            + (column - region.x()) as usize]
                    }
                    _ => matte,
                };
                let actual = reference.pixels()[y as usize * target_stride + x as usize];
                if actual != expected {
                    tracing::error!(
                        operation = "canvas_proof_pixel_mismatch",
                        backend = "gpu",
                        x,
                        y,
                        maximum_channel_difference = maximum_channel_difference(actual, expected),
                        "Retained crop differs from complete reconstruction"
                    );
                    return Err(WorkError::invalid(
                        "Retained GPU backing crop differs from complete DAG reconstruction",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn mapped_bounds(
    start: u32,
    end: u32,
    mut map: impl FnMut(u32) -> Option<u32>,
) -> Option<(u32, u32)> {
    let mut bounds: Option<(u32, u32)> = None;
    for value in start..end {
        if let Some(mapped) = map(value) {
            bounds = Some(match bounds {
                Some((minimum, maximum)) => (minimum.min(mapped), maximum.max(mapped)),
                None => (mapped, mapped),
            });
        }
    }
    bounds
}

/// Describe numerical disagreement without recording image pixels or source content.
fn maximum_channel_difference(left: u32, right: u32) -> u8 {
    left.to_be_bytes()
        .into_iter()
        .zip(right.to_be_bytes())
        .map(|(left, right)| left.abs_diff(right))
        .fold(0, u8::max)
}

/// Difference cumulative source/operation counters without attributing earlier work to a new frame.
pub(super) fn difference(current: EvaluationStats, earlier: EvaluationStats) -> (u64, u64) {
    (
        current.computed_pixels.saturating_sub(earlier.computed_pixels),
        current.source_pixels.saturating_sub(earlier.source_pixels),
    )
}
