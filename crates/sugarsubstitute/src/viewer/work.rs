// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare original assets and viewport pixels exclusively on execution-owned workers.
use super::work_error::WorkError;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use sugarsubstitute_canvas::{
    Document, DocumentEdit, DocumentSnapshot, FrameStamp, Layer, LayerContent, LayerId,
    RenderRequest, Viewport,
};
use sugarsubstitute_canvas_render::{CpuRenderer, CpuSurface, PreparedUpdate};
use sugarsubstitute_contracts::execution::{
    CpuStep, CpuTask, JobContext, MemoryClaim, MemoryLease,
};
use sugarsubstitute_image_io::{CacheReuse, SharedRaster, import_png};
use sugarsubstitute_image_ops::{AssetId, Extent, Filter, ImageError, Pixel, RasterSource, Region};
use sugarsubstitute_supervision::cache::{CachePolicy, CacheReader, RuntimeCache};

pub(super) const RETAINED_BYTES: u64 = 160 * 1024 * 1024;
pub(super) const IMPORT_BYTES: u64 = 48 * 1024 * 1024;
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
    pub(super) memory: MemoryLease,
    pub(super) cache: RuntimeCache,
    pub(super) engine: Arc<Mutex<Engine>>,
    pub(super) _metadata: Arc<MemoryClaim>,
}
impl Clone for Resources {
    fn clone(&self) -> Self {
        Self {
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
    renderer: Option<CpuRenderer<SourcePort>>,
    source_revision: Option<u64>,
}

/// Initialize one standalone cache scope and original admission without native window effects.
pub(super) fn initialize(
    anchor: PathBuf,
    context: &JobContext,
) -> Result<(Resources, Scene), WorkError> {
    let memory = context.memory().clone();
    let metadata = Arc::new(memory.claim(1024 * 1024)?);
    let cache = RuntimeCache::open(&anchor, CachePolicy::default(), context)?;
    let document = Document::new(1, Extent::new(1, 1)?, &memory)?;
    let engine = Arc::new(Mutex::new(Engine { renderer: None, source_revision: None }));
    Ok((
        Resources { memory, cache, engine, _metadata: metadata },
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
    pub(super) update: PreparedUpdate,
    pub(super) replacement: Option<CpuSurface>,
    pub(super) view: Viewport,
    pub(super) filter: Filter,
    pub(super) document: (u64, u64),
    pub(super) elapsed: std::time::Duration,
    pub(super) stats: sugarsubstitute_image_ops::EvaluationStats,
    pub(super) source_revision: u64,
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
    pub(super) finished: bool,
}
impl CpuTask<Packet, WorkError> for Prepare {
    fn step(&mut self, context: &JobContext) -> Result<CpuStep<Packet>, WorkError> {
        if self.finished {
            return Err(WorkError::invalid("Canvas request was already prepared"));
        }
        self.finished = true;
        let started = std::time::Instant::now();
        let mut engine = self
            .resources
            .engine
            .lock()
            .map_err(|_| WorkError::invalid("Canvas preparation state failed"))?;
        if engine.source_revision != Some(self.source_revision) {
            let renderer =
                CpuRenderer::new(SourcePort(self.source.clone()), 512, &self.resources.memory)?;
            engine.renderer = Some(renderer);
            engine.source_revision = Some(self.source_revision);
        }
        let renderer = engine
            .renderer
            .as_mut()
            .ok_or_else(|| WorkError::invalid("Canvas renderer is unavailable"))?;
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
            update,
            replacement,
            view: self.view,
            filter: self.filter,
            document: (self.document.identity(), self.document.revision()),
            elapsed: started.elapsed(),
            stats: renderer.stats(),
            source_revision: self.source_revision,
        }))
    }
}
