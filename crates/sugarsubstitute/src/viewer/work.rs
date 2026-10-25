// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare original assets and viewport pixels exclusively on execution-owned workers.
use super::work_error::WorkError;
mod gpu;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use sugarsubstitute_canvas::{
    Document, DocumentEdit, DocumentSnapshot, FrameStamp, Layer, LayerContent, LayerId,
    RenderRequest, Viewport,
};
use sugarsubstitute_canvas_render::{
    CpuRenderer, CpuSurface, GpuRenderer, GpuSurface, PreparedUpdate, SurfaceWork,
};
use sugarsubstitute_contracts::execution::{
    CpuStep, CpuTask, JobContext, MemoryClaim, MemoryLease,
};
use sugarsubstitute_image_gpu::{GpuDevice, WindowFrame, WindowSurface};
use sugarsubstitute_image_io::{CacheReuse, SharedRaster, import_png};
use sugarsubstitute_image_ops::{AssetId, Extent, Filter, ImageError, Pixel, RasterSource, Region};
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
    pub(super) original: Option<PathBuf>,
}
/// Original retained admission is shared by cache custody, source data, DAG reuse and UI buffers.
pub(super) struct Resources {
    pub(super) capture: bool,
    pub(super) memory: MemoryLease,
    pub(super) cache: RuntimeCache,
    pub(super) engine: Arc<Mutex<Engine>>,
    pub(super) _metadata: Arc<MemoryClaim>,
}
impl Clone for Resources {
    fn clone(&self) -> Self {
        Self {
            capture: self.capture,
            memory: self.memory.clone(),
            cache: self.cache.clone(),
            engine: self.engine.clone(),
            _metadata: self._metadata.clone(),
        }
    }
}
/// Share only immutable source identity; region reads execute on the admitted preparation worker.
pub(super) struct SourcePort(pub(super) Option<Source>);
impl RasterSource for SourcePort {
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), ImageError> {
        self.0
            .as_mut()
            .ok_or(ImageError::Invalid("Canvas source is unavailable"))?
            .read(asset, region, output, context)
    }
}
/// Mutable DAG reuse state belongs to preparation workers; the native thread never locks it.
pub(super) struct Engine {
    renderer: Option<Renderer>,
    source_revision: Option<u64>,
    device: Option<Arc<GpuDevice>>,
    window: Option<WindowSurface>,
}
enum Renderer {
    Cpu(Box<CpuRenderer<SourcePort>>),
    Gpu(Box<GpuRenderer<SourcePort>>),
}
impl Resources {
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
            window,
        }));
        self
    }
}

/// Initialize one standalone cache scope and original admission without native window effects.
pub(super) fn initialize(
    anchor: PathBuf,
    capture: bool,
    context: &JobContext,
) -> Result<(Resources, Scene), WorkError> {
    let memory = context.memory().clone();
    let metadata = Arc::new(memory.claim(1024 * 1024)?);
    let cache = RuntimeCache::open(&anchor, CachePolicy::default(), context)?;
    let document = Document::new(1, Extent::new(1, 1)?, &memory)?;
    let engine = Arc::new(Mutex::new(Engine {
        renderer: None,
        source_revision: None,
        device: None,
        window: None,
    }));
    Ok((
        Resources { memory, cache, engine, _metadata: metadata, capture },
        Scene { document, source: None, source_revision: 0, original: None },
    ))
}
/// Hash/decode an explicit original; image storage never becomes authoritative document data.
pub(super) fn load(
    path: PathBuf,
    revision: u64,
    reuse: CacheReuse,
    resources: &Resources,
    context: &JobContext,
) -> Result<Scene, WorkError> {
    let now = sugarsubstitute_supervision::cache::utc_seconds()?;
    let raster =
        import_png(&path, resources.cache.clone(), reuse, &resources.memory, now, context)?;
    let source = SharedRaster::new(raster, &resources.memory)?;
    let descriptor = source.descriptor();
    let extent = descriptor.layout().extent();
    let mut document = Document::new(revision, extent, &resources.memory)?;
    let layer =
        Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: descriptor.asset(), extent })?;
    document.apply(DocumentEdit::Insert { index: 0, layer })?;
    Ok(Scene { document, source: Some(source), source_revision: revision, original: Some(path) })
}
/// One immutable prepared request plus optional newly admitted UI backing for physical resize.
pub(super) struct Packet {
    pub(super) pixels: Pixels,
    pub(super) damage: sugarsubstitute_canvas::DamagePlan,
    pub(super) base: Option<FrameStamp>,
    pub(super) view: Viewport,
    pub(super) filter: Filter,
    pub(super) document: (u64, u64),
    pub(super) elapsed: std::time::Duration,
    pub(super) stats: sugarsubstitute_image_ops::EvaluationStats,
    pub(super) source_revision: u64,
}
pub(super) enum Pixels {
    Cpu { update: Box<PreparedUpdate>, replacement: Option<CpuSurface> },
    Gpu(GpuPacket),
}
pub(super) struct GpuPacket {
    pub(super) surface: GpuSurface,
    pub(super) presentation: Option<WindowFrame>,
    pub(super) snapshot: Option<CpuSurface>,
    pub(super) work: SurfaceWork,
    pub(super) retry_presentation: bool,
    pub(super) native_verified: bool,
}
/// Own all inputs for one accepted CPU request; subsequent navigation intents remain outside it.
pub(super) struct Prepare {
    pub(super) resources: Resources,
    pub(super) document: DocumentSnapshot,
    pub(super) source: Option<Source>,
    pub(super) source_revision: u64,
    pub(super) view: Viewport,
    pub(super) filter: Filter,
    pub(super) target: FrameStamp,
    pub(super) base: Option<FrameStamp>,
    pub(super) damage: sugarsubstitute_canvas::DamagePlan,
    pub(super) replace_surface: bool,
    pub(super) gpu_surface: Option<GpuSurface>,
    pub(super) finished: bool,
}
impl CpuTask<Packet, WorkError> for Prepare {
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<Packet>, WorkError> {
        if self.finished {
            return Err(WorkError::invalid("Canvas request was already prepared"));
        }
        self.finished = true;
        let started = std::time::Instant::now();
        let handle = self.resources.engine.clone();
        let mut engine =
            handle.lock().map_err(|_| WorkError::invalid("Canvas preparation state failed"))?;
        if engine.source_revision != Some(self.source_revision) {
            let renderer = match &engine.device {
                Some(device) => Renderer::Gpu(Box::new(GpuRenderer::new(
                    SourcePort(self.source.clone()),
                    512,
                    device.clone(),
                    &self.resources.memory,
                    context,
                )?)),
                None => Renderer::Cpu(Box::new(CpuRenderer::new(
                    SourcePort(self.source.clone()),
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
            return gpu::prepare(self, &mut engine, started, context).map(CpuStep::Complete);
        }
        let Renderer::Cpu(renderer) = renderer else {
            return Err(WorkError::invalid("Canvas renderer and startup backend differ"));
        };
        let prepared = renderer.prepare(
            RenderRequest {
                document: &self.document,
                view: self.view,
                filter: self.filter,
                matte: Pixel::linear(0.018, 0.018, 0.018, 1.0)?,
                target: self.target,
                base: self.base,
                damage: self.damage,
            },
            context,
        );
        if started.elapsed() >= std::time::Duration::from_secs(1) || prepared.is_err() {
            let stats = renderer.stats();
            tracing::info!(operation="canvas_frame_preparation", document=self.document.identity(), revision=self.document.revision(), generation=?self.target.generation, elapsed_milliseconds=started.elapsed().as_millis(), computed_pixels=stats.computed_pixels, source_pixels=stats.source_pixels, succeeded=prepared.is_ok(), "Canvas DAG preparation reached its observed outcome");
        }
        let update = prepared?;
        let replacement = if self.replace_surface {
            Some(CpuSurface::new(self.target.surface, self.view.surface(), &self.resources.memory)?)
        } else {
            None
        };
        Ok(CpuStep::Complete(Packet {
            pixels: Pixels::Cpu { update: Box::new(update), replacement },
            damage: self.damage,
            base: self.base,
            view: self.view,
            filter: self.filter,
            document: (self.document.identity(), self.document.revision()),
            elapsed: started.elapsed(),
            stats: renderer.stats(),
            source_revision: self.source_revision,
        }))
    }
}
