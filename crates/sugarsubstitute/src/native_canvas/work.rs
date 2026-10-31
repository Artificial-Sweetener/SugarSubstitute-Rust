// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare original assets and viewport pixels exclusively on execution-owned workers.
use super::{coverage::BackingTier, work_error::WorkError};
mod cpu;
mod display;
mod gpu;
#[cfg(feature = "native-composition")]
pub(super) mod preview;
pub(super) use display::{
    CpuUnderlay, Draw, DrawInput, DrawPixels, DrawSource, GpuUnderlay, UnderlayEvidence,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use sugarsubstitute_canvas::{
    Comparison, Document, DocumentEdit, DocumentSnapshot, FrameStamp, Layer, LayerContent, LayerId,
    RenderRequest, Viewport,
};
use sugarsubstitute_canvas_render::{
    CpuPreparation, CpuRenderer, CpuSurface, GpuRenderer, GpuSurface, PreparedUpdate,
    ProjectionBuffers, SurfaceWork,
};
use sugarsubstitute_contracts::execution::{
    CpuStep, CpuTask, JobContext, MemoryClaim, MemoryLease,
};
use sugarsubstitute_image_gpu::{GpuDevice, WindowFrame, WindowSurface};
use sugarsubstitute_image_io::{ArchiveStats, CacheReuse, ImageSource, SharedRaster};
use sugarsubstitute_image_ops::{
    AssetId, Extent, ImageError, Pixel, RasterSampleRequest, RasterSource, Reconstruction, Region,
};
use sugarsubstitute_supervision::cache::{CachePolicy, CacheReader, RuntimeCache};

pub(super) const RETAINED_BYTES: u64 = 160 * 1024 * 1024;
pub(super) const GPU_RETAINED_BYTES: u64 = 304 * 1024 * 1024;
pub(super) const IMPORT_BYTES: u64 = 48 * 1024 * 1024;
pub(super) const GPU_PREPARE_BYTES: u64 = 32 * 1024 * 1024;
pub(super) const GPU_PROOF_BYTES: u64 = 40 * 1024 * 1024;
pub(super) type Source = SharedRaster<CacheReader<'static>>;

/// Authored document and immutable original source handle; camera state lives outside the document.
pub(super) struct Scene {
    pub(super) document: Document,
    pub(super) source: Option<Source>,
    pub(super) source_revision: u64,
    pub(super) original: Option<ImageSource>,
    /// Inspection-only comparison original loaded with this source revision; never authored.
    pub(super) compared: Option<Box<Compared>>,
}
/// A comparison original retained beside the document source for the same selection.
pub(super) struct Compared {
    pub(super) source: Source,
    pub(super) _original: ImageSource,
}
impl Compared {
    /// Describe the comparison raster for a document-space wipe boundary.
    pub(super) fn comparison(
        &self,
        orientation: sugarsubstitute_canvas::WipeOrientation,
        boundary: f64,
    ) -> Result<Comparison, sugarsubstitute_canvas::CanvasError> {
        let descriptor = self.source.descriptor();
        Comparison::new(descriptor.asset(), descriptor.layout().extent(), orientation, boundary)
    }
}
/// One selected original and an optional comparison original, admitted and loaded together.
pub(crate) struct Originals {
    pub(crate) primary: ImageSource,
    pub(crate) compared: Option<ImageSource>,
}
impl Originals {
    /// Select one original without a comparison.
    pub(crate) const fn single(primary: ImageSource) -> Self {
        Self { primary, compared: None }
    }
    /// Retain both originals until a storage worker can dispose of either one.
    pub(super) fn requires_worker_retirement(&self) -> bool {
        self.primary.requires_worker_retirement()
            || self.compared.as_ref().is_some_and(ImageSource::requires_worker_retirement)
    }
}
/// Original retained admission is shared by cache custody, source data, DAG reuse and UI buffers.
pub(super) struct Resources {
    pub(super) capture: bool,
    verify_native: bool,
    pub(super) memory: MemoryLease,
    pub(super) cache: RuntimeCache,
    pub(super) engine: Arc<Mutex<Engine>>,
    window: Arc<Mutex<Option<WindowSurface>>>,
    pub(super) _metadata: Arc<MemoryClaim>,
}
impl Clone for Resources {
    fn clone(&self) -> Self {
        Self {
            capture: self.capture,
            verify_native: self.verify_native,
            memory: self.memory.clone(),
            cache: self.cache.clone(),
            engine: self.engine.clone(),
            window: self.window.clone(),
            _metadata: self._metadata.clone(),
        }
    }
}
/// Share only immutable source identities; region reads execute on the admitted preparation worker
/// and dispatch to the original whose content identity the image graph requested.
pub(super) struct SourcePort(pub(super) Vec<Source>);
impl SourcePort {
    fn source(&mut self, asset: AssetId) -> Result<&mut Source, ImageError> {
        self.0
            .iter_mut()
            .find(|source| source.descriptor().asset() == asset)
            .ok_or(ImageError::Invalid("Canvas source is unavailable"))
    }
}
impl RasterSource for SourcePort {
    fn read_sample_batch(
        &mut self,
        asset: AssetId,
        requests: &mut [RasterSampleRequest<'_>],
        context: &JobContext,
    ) -> Result<(), ImageError> {
        self.source(asset)?.read_sample_batch(asset, requests, context)
    }

    fn read_samples(
        &mut self,
        asset: AssetId,
        samples: &sugarsubstitute_image_ops::RasterSamples,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), ImageError> {
        self.source(asset)?.read_samples(asset, samples, output, context)
    }

    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), ImageError> {
        self.source(asset)?.read(asset, region, output, context)
    }
}
/// Mutable DAG reuse state belongs to preparation workers; the native thread never locks it.
pub(super) struct Engine {
    renderer: Option<Renderer>,
    source_revision: Option<u64>,
    device: Option<Arc<GpuDevice>>,
}
enum Renderer {
    Cpu(Box<CpuRenderer<SourcePort>>),
    Gpu(Box<GpuRenderer<SourcePort>>),
}
impl Resources {
    /// Release reconstructible image tiles before an independent proof evaluator shares this lease.
    /// Call only on an admitted worker after viewer preparation has reached a terminal outcome.
    pub(super) fn release_graph_tiles(&self) -> Result<(), WorkError> {
        let mut engine = self
            .engine
            .lock()
            .map_err(|_| WorkError::invalid("Canvas preparation state failed"))?;
        match engine.renderer.as_mut() {
            Some(Renderer::Cpu(renderer)) => renderer.clear_graph_tiles(),
            Some(Renderer::Gpu(renderer)) => while renderer.evict_one() {},
            None => {}
        }
        Ok(())
    }
    /// Bind the startup-selected native device before any frame preparation has begun.
    pub(super) fn with_gpu(
        mut self,
        device: Arc<GpuDevice>,
        window: Option<WindowSurface>,
    ) -> Self {
        self.engine = Arc::new(Mutex::new(Engine {
            renderer: None,
            source_revision: None,
            device: Some(device),
        }));
        self.window = Arc::new(Mutex::new(window));
        self
    }
}

/// Initialize one standalone cache scope and original admission without native window effects.
pub(super) fn initialize(
    anchor: PathBuf,
    capture: bool,
    verify_native: bool,
    context: &JobContext,
) -> Result<(Resources, Scene), WorkError> {
    let memory = context.memory().clone();
    let metadata = Arc::new(memory.claim(1024 * 1024)?);
    let cache = RuntimeCache::open(&anchor, CachePolicy::default(), context)?;
    let document = Document::new(1, Extent::new(1, 1)?, &memory)?;
    let engine =
        Arc::new(Mutex::new(Engine { renderer: None, source_revision: None, device: None }));
    Ok((
        Resources {
            memory,
            cache,
            engine,
            window: Arc::new(Mutex::new(None)),
            _metadata: metadata,
            capture,
            verify_native,
        },
        Scene { document, source: None, source_revision: 0, original: None, compared: None },
    ))
}
/// Hash/decode explicit originals; image storage never becomes authoritative document data.
/// A comparison original is imported under the same admission and becomes inspection state only.
pub(super) fn load(
    originals: Originals,
    revision: u64,
    reuse: CacheReuse,
    resources: &Resources,
    context: &JobContext,
) -> Result<Scene, WorkError> {
    let now = sugarsubstitute_supervision::cache::utc_seconds()?;
    let Originals { primary: original, compared } = originals;
    let raster =
        original.import_image(resources.cache.clone(), reuse, &resources.memory, now, context)?;
    let source = SharedRaster::new(raster, &resources.memory)?;
    let compared = match compared {
        Some(compared) => {
            let raster = compared.import_image(
                resources.cache.clone(),
                reuse,
                &resources.memory,
                now,
                context,
            )?;
            Some(Box::new(Compared {
                source: SharedRaster::new(raster, &resources.memory)?,
                _original: compared,
            }))
        }
        None => None,
    };
    let descriptor = source.descriptor();
    let extent = descriptor.layout().extent();
    let mut document = Document::new(revision, extent, &resources.memory)?;
    let layer =
        Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: descriptor.asset(), extent })?;
    document.apply(DocumentEdit::Insert { index: 0, layer })?;
    Ok(Scene {
        document,
        source: Some(source),
        source_revision: revision,
        original: Some(original),
        compared,
    })
}
/// One immutable prepared request plus optional newly admitted UI backing for physical resize.
pub(super) struct Packet {
    pub(super) sampling_scale: f64,
    pub(super) tier: BackingTier,
    pub(super) reconstruction: Reconstruction,
    pub(super) display_buffers: Option<ProjectionBuffers>,
    pub(super) pixels: Pixels,
    pub(super) view: Viewport,
    pub(super) document: (u64, u64),
    pub(super) elapsed: std::time::Duration,
    pub(super) stats: sugarsubstitute_image_ops::EvaluationStats,
    pub(super) source_revision: u64,
    pub(super) source_io: Option<ArchiveStats>,
}
pub(super) enum Pixels {
    Cpu(CpuPacket),
    Gpu(GpuPacket),
}
/// A first/resize frame adopts complete pixels; repairs depend on the current retained frame.
pub(super) enum CpuPacket {
    Replace { surface: CpuSurface },
    Repair(Box<PreparedUpdate>),
}
pub(super) struct GpuPacket {
    pub(super) surface: GpuSurface,
    pub(super) snapshot: Option<CpuSurface>,
    pub(super) work: SurfaceWork,
}
/// Own all inputs for one accepted CPU request; subsequent navigation intents remain outside it.
pub(super) struct Prepare {
    pub(super) tier: BackingTier,
    pub(super) reconstruction: Reconstruction,
    pub(super) resources: Resources,
    pub(super) document: DocumentSnapshot,
    pub(super) source: Option<Source>,
    pub(super) compared: Option<Source>,
    pub(super) comparison: Option<Comparison>,
    pub(super) source_revision: u64,
    pub(super) view: Viewport,
    pub(super) target: FrameStamp,
    pub(super) base: Option<FrameStamp>,
    pub(super) damage: sugarsubstitute_canvas::DamagePlan,
    pub(super) replace_surface: bool,
    pub(super) replace_buffers: bool,
    pub(super) display_extent: Extent,
    pub(super) gpu_surface: Option<GpuSurface>,
    pub(super) finished: bool,
    pub(super) cpu: Option<CpuPreparation>,
    pub(super) display_buffers: Option<ProjectionBuffers>,
    pub(super) started: Option<std::time::Instant>,
    pub(super) source_stats_before: Option<ArchiveStats>,
}
impl CpuTask<Packet, WorkError> for Prepare {
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<Packet>, WorkError> {
        if self.finished {
            return Err(WorkError::invalid("Canvas request was already prepared"));
        }
        if self.source_stats_before.is_none() {
            self.source_stats_before = self.source.as_ref().map(Source::stats).transpose()?;
        }
        let started = *self.started.get_or_insert_with(std::time::Instant::now);
        let handle = self.resources.engine.clone();
        let mut engine =
            handle.lock().map_err(|_| WorkError::invalid("Canvas preparation state failed"))?;
        if engine.source_revision != Some(self.source_revision) {
            let renderer = match &engine.device {
                Some(device) => Renderer::Gpu(Box::new(GpuRenderer::new(
                    SourcePort(self.source.iter().chain(&self.compared).cloned().collect()),
                    512,
                    device.clone(),
                    &self.resources.memory,
                    context,
                )?)),
                None => Renderer::Cpu(Box::new(CpuRenderer::new(
                    SourcePort(self.source.iter().chain(&self.compared).cloned().collect()),
                    512,
                    &self.resources.memory,
                )?)),
            };
            engine.renderer = Some(renderer);
            engine.source_revision = Some(self.source_revision);
        }
        let renderer = engine
            .renderer
            .as_mut()
            .ok_or_else(|| WorkError::invalid("Canvas renderer is unavailable"))?;
        if matches!(renderer, Renderer::Gpu(_)) {
            self.finished = true;
            return gpu::prepare(self, &mut engine, started, context).map(CpuStep::Complete);
        }
        let Renderer::Cpu(renderer) = renderer else {
            return Err(WorkError::invalid("Canvas renderer and startup backend differ"));
        };
        cpu::step(self, renderer, started, context)
    }
}
impl Prepare {
    /// Worker-observed archive work attributable to this preparation.
    fn source_io(&self) -> Result<Option<ArchiveStats>, WorkError> {
        match (&self.source, self.source_stats_before) {
            (Some(source), Some(before)) => Ok(Some(source.stats()?.since(before))),
            (None, None) => Ok(None),
            _ => Err(WorkError::invalid("Canvas source statistics state is incomplete")),
        }
    }
}
