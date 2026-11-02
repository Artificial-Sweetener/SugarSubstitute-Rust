// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare original assets and viewport pixels exclusively on execution-owned workers.
use super::{budget, coverage::BackingTier, work_error::WorkError};
mod cpu;
mod display;
mod gpu;
#[cfg(feature = "native-composition")]
pub(super) mod preview;
#[cfg(all(windows, feature = "native-composition"))]
pub(super) use display::{CompositionFrame, CompositionTarget};
pub(super) use display::{
    CpuUnderlay, DisplayTarget, Draw, DrawInput, DrawPixels, DrawSource, GpuUnderlay, NativeFrame,
    UnderlayEvidence,
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
use sugarsubstitute_image_gpu::{GpuDevice, WindowSurface};
use sugarsubstitute_image_io::{ArchiveStats, CacheReuse, ImageSource, SharedRaster};
use sugarsubstitute_image_ops::{
    AssetId, Extent, ImageError, Pixel, RasterSampleRequest, RasterSource, Reconstruction, Region,
};
use sugarsubstitute_supervision::cache::{CachePolicy, CacheReader, RuntimeCache};

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
    pub(super) compared: Option<Box<Companion>>,
    /// Further originals referenced by placed overview layers, beside the primary `source`.
    pub(super) placed: Vec<Companion>,
}
/// A further original retained beside the document source for the same selection, for a
/// comparison or one placed overview cell.
pub(super) struct Companion {
    pub(super) source: Source,
    pub(super) _original: ImageSource,
}
impl Companion {
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
/// One selected original with an optional comparison original or overview grid, admitted and
/// loaded together as one selection.
pub(crate) struct Originals {
    pub(crate) primary: ImageSource,
    pub(crate) compared: Option<ImageSource>,
    pub(crate) overview: Option<OverviewOriginals>,
}
/// Originals composed into one document of the viewport's physical extent. Each cell is a
/// physical-pixel `[left, top, width, height]` rectangle; its image is fitted inside, centered.
pub(crate) struct OverviewOriginals {
    pub(crate) extent: Extent,
    pub(crate) primary_cell: [f64; 4],
    pub(crate) cells: Vec<(ImageSource, [f64; 4])>,
}
impl Originals {
    /// Select one original without a comparison.
    pub(crate) const fn single(primary: ImageSource) -> Self {
        Self { primary, compared: None, overview: None }
    }
    /// Retain every original until a storage worker can dispose of any one of them.
    pub(super) fn requires_worker_retirement(&self) -> bool {
        self.primary.requires_worker_retirement()
            || self.compared.as_ref().is_some_and(ImageSource::requires_worker_retirement)
            || self.overview.as_ref().is_some_and(|overview| {
                overview.cells.iter().any(|(original, _)| original.requires_worker_retirement())
            })
    }
}
/// Host-identified overview cells before acquisition: the first cell is the primary selection.
/// Cells are physical-pixel `[left, top, width, height]` rectangles inside `extent`.
#[cfg(feature = "native-composition")]
pub(crate) struct OverviewSelection<T> {
    pub(crate) extent: Extent,
    pub(crate) primary: (T, [f64; 4]),
    pub(crate) cells: Vec<(T, [f64; 4])>,
}
/// Fit `image` inside `cell` without cropping, centered, as the web overview's `contain` does.
fn contain(image: Extent, cell: [f64; 4]) -> [f64; 4] {
    let [left, top, width, height] = cell;
    let (image_width, image_height) = (f64::from(image.width()), f64::from(image.height()));
    let scale = (width / image_width).min(height / image_height);
    let (fitted_width, fitted_height) = (image_width * scale, image_height * scale);
    [
        left + (width - fitted_width) / 2.0,
        top + (height - fitted_height) / 2.0,
        fitted_width,
        fitted_height,
    ]
}
/// Original retained admission is shared by cache custody, source data, DAG reuse and UI buffers.
pub(super) struct Resources {
    pub(super) capture: bool,
    verify_native: bool,
    pub(super) memory: MemoryLease,
    /// Storage carved from the same admission for the graphics device and composition
    /// surfaces, which the UI thread claims without being able to evict cached tiles.
    pub(super) presentation: MemoryLease,
    pub(super) cache: RuntimeCache,
    pub(super) engine: Arc<Mutex<Engine>>,
    display: Arc<Mutex<Option<DisplayTarget>>>,
    pub(super) _metadata: Arc<MemoryClaim>,
    /// The selected device, held apart from the engine so the native thread reads its video
    /// memory budget without locking preparation state.
    budget_device: Option<Arc<GpuDevice>>,
}
impl Clone for Resources {
    fn clone(&self) -> Self {
        Self {
            capture: self.capture,
            verify_native: self.verify_native,
            memory: self.memory.clone(),
            presentation: self.presentation.clone(),
            cache: self.cache.clone(),
            engine: self.engine.clone(),
            display: self.display.clone(),
            _metadata: self._metadata.clone(),
            budget_device: self.budget_device.clone(),
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
        self.budget_device = Some(device.clone());
        self.engine = Arc::new(Mutex::new(Engine {
            renderer: None,
            source_revision: None,
            device: Some(device),
        }));
        self.display = Arc::new(Mutex::new(window.map(DisplayTarget::Window)));
        self
    }
    /// The selected device's video memory budget, when a GPU renders and its driver reports one.
    #[cfg(all(windows, feature = "native-composition"))]
    pub(super) fn video_memory_budget(
        &self,
    ) -> Option<sugarsubstitute_image_gpu::VideoMemoryBudget> {
        self.budget_device.as_ref().and_then(|device| device.video_memory_budget())
    }
    /// Bind the selected device and its main-window composition target before preparation.
    #[cfg(all(windows, feature = "native-composition"))]
    pub(super) fn with_composition(
        mut self,
        device: Arc<GpuDevice>,
        target: CompositionTarget,
    ) -> Self {
        self.budget_device = Some(device.clone());
        self.engine = Arc::new(Mutex::new(Engine {
            renderer: None,
            source_revision: None,
            device: Some(device),
        }));
        self.display = Arc::new(Mutex::new(Some(DisplayTarget::Composition(Box::new(target)))));
        self
    }
}

/// Initialize one standalone cache scope and original admission without native window effects.
pub(super) fn initialize(
    anchor: PathBuf,
    capture: bool,
    verify_native: bool,
    presentation_bytes: u64,
    context: &JobContext,
) -> Result<(Resources, Scene), WorkError> {
    let memory = context.memory().clone();
    let presentation = memory.claim(presentation_bytes)?.into_lease();
    let metadata = Arc::new(memory.claim(1024 * 1024)?);
    let cache = RuntimeCache::open(&anchor, CachePolicy::default(), context)?;
    #[cfg(feature = "native-composition")]
    super::drag_staging::clean(&cache, sugarsubstitute_supervision::cache::utc_seconds()?, context);
    let document = Document::new(1, Extent::new(1, 1)?, &memory)?;
    let engine =
        Arc::new(Mutex::new(Engine { renderer: None, source_revision: None, device: None }));
    Ok((
        Resources {
            memory,
            presentation,
            cache,
            engine,
            display: Arc::new(Mutex::new(None)),
            _metadata: metadata,
            capture,
            verify_native,
            budget_device: None,
        },
        Scene {
            document,
            source: None,
            source_revision: 0,
            original: None,
            compared: None,
            placed: Vec::new(),
        },
    ))
}
/// Hash/decode explicit originals; image storage never becomes authoritative document data.
/// A comparison original or further overview cells are imported under the same admission;
/// a comparison stays inspection state and overview cells become placed layers.
pub(super) fn load(
    originals: Originals,
    revision: u64,
    reuse: CacheReuse,
    resources: &Resources,
    context: &JobContext,
) -> Result<Scene, WorkError> {
    let now = sugarsubstitute_supervision::cache::utc_seconds()?;
    let Originals { primary: original, compared, overview } = originals;
    let import = |original: &ImageSource| -> Result<Source, WorkError> {
        let raster = original.import_image(
            resources.cache.clone(),
            reuse,
            &resources.memory,
            now,
            context,
        )?;
        Ok(SharedRaster::new(raster, &resources.memory)?)
    };
    let source = import(&original)?;
    if let Some(overview) = overview {
        if compared.is_some() {
            return Err(WorkError::invalid("An overview cannot also carry a comparison"));
        }
        return overview_scene(source, original, overview, revision, &import, resources);
    }
    let compared = match compared {
        Some(compared) => {
            Some(Box::new(Companion { source: import(&compared)?, _original: compared }))
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
        placed: Vec::new(),
    })
}
/// Compose the primary and every further cell original into one viewport-sized document of
/// placed layers; each original keeps its own pixels and is fitted inside its cell.
fn overview_scene(
    source: Source,
    original: ImageSource,
    overview: OverviewOriginals,
    revision: u64,
    import: &dyn Fn(&ImageSource) -> Result<Source, WorkError>,
    resources: &Resources,
) -> Result<Scene, WorkError> {
    let OverviewOriginals { extent, primary_cell, cells } = overview;
    let mut document = Document::new(revision, extent, &resources.memory)?;
    place(&mut document, 0, &source, primary_cell)?;
    let mut placed = Vec::with_capacity(cells.len());
    for (index, (cell_original, cell)) in cells.into_iter().enumerate() {
        let cell_source = import(&cell_original)?;
        place(&mut document, index + 1, &cell_source, cell)?;
        placed.push(Companion { source: cell_source, _original: cell_original });
    }
    Ok(Scene {
        document,
        source: Some(source),
        source_revision: revision,
        original: Some(original),
        compared: None,
        placed,
    })
}

/// Insert one original fitted inside its cell as the next placed layer.
fn place(
    document: &mut Document,
    index: usize,
    source: &Source,
    cell: [f64; 4],
) -> Result<(), WorkError> {
    let descriptor = source.descriptor();
    let image = descriptor.layout().extent();
    let id = u64::try_from(index + 1).map_err(|_| WorkError::invalid("Overview cell identity"))?;
    let layer = Layer::new(
        LayerId::new(id)?,
        LayerContent::PlacedRaster {
            asset: descriptor.asset(),
            extent: image,
            bounds: contain(image, cell),
        },
    )?;
    document.apply(DocumentEdit::Insert { index, layer })?;
    Ok(())
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
    pub(super) placed: Vec<Source>,
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
                    SourcePort(
                        self.source
                            .iter()
                            .chain(&self.compared)
                            .chain(&self.placed)
                            .cloned()
                            .collect(),
                    ),
                    budget::RENDERER_TILES,
                    device.clone(),
                    &self.resources.memory,
                    context,
                )?)),
                None => Renderer::Cpu(Box::new(CpuRenderer::new(
                    SourcePort(
                        self.source
                            .iter()
                            .chain(&self.compared)
                            .chain(&self.placed)
                            .cloned()
                            .collect(),
                    ),
                    budget::RENDERER_TILES,
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
