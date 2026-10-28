// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate one camera/document lifetime with typed shared-execution jobs and worker retirement.
#[cfg(feature = "native-composition")]
pub(crate) mod acquisition;
mod activity;
mod attachment;
mod backing;
mod closing;
mod completion;
mod display;
mod document;
mod gpu;
#[cfg(all(windows, feature = "native-composition"))]
mod host_graphics;
mod native;
mod preparation;
mod presentation;
mod proof;
pub(crate) mod runtime;
mod source;
mod startup;
#[cfg(test)]
mod tests;
use self::display::DisplayEvidence;
use self::runtime::Runtime;
use super::{
    config::{Config, InitialSource},
    coverage::{self, BackingTier},
    work::{self, Packet, Resources, Scene},
    work_error::WorkError,
};
use crate::{diagnostics::DiagnosticHealth, job_sequence::JobSequence};
use std::{error::Error, path::PathBuf, sync::Arc, time::Duration};
use sugarsubstitute_canvas::{
    BackingCoverage, DamagePlan, FrameStamp, Navigation, ToolManager, Viewport,
};
use sugarsubstitute_canvas_render::{
    CpuProjection, CpuSurface, GpuSurface, PresentationMode, PresentationPacer,
    PresentationReadiness, ProjectionBuffer,
};
use sugarsubstitute_contracts::{
    execution::*,
    localization::{messages, viewer::ViewerMessages},
};
use sugarsubstitute_desktop::canvas::GpuPresentation;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
use sugarsubstitute_image_gpu::{AdapterDescriptor, GpuDevice, WindowFrame};
use sugarsubstitute_image_io::{ArchiveStats, CacheReuse, ImageSource};
use sugarsubstitute_image_ops::{AssetId, Reconstruction};

/// Latest explicit open intent; replacing it never turns an earlier completion into current content.
struct Open {
    source: OpenSource,
    intent: u64,
    reuse: CacheReuse,
    expected: Option<AssetId>,
}

/// An explicit original or an inert host adapter that acquires one on a storage worker.
enum OpenSource {
    Image(ImageSource),
    #[cfg(feature = "native-composition")]
    Acquired(Arc<dyn acquisition::SourceAcquirer>),
}

impl OpenSource {
    /// Retain source custody until a storage worker can dispose it.
    fn requires_worker_retirement(&self) -> bool {
        match self {
            Self::Image(source) => source.retains_native_storage(),
            #[cfg(feature = "native-composition")]
            Self::Acquired(_) => true,
        }
    }
}
/// One bounded admitted disposal transaction; payload fields retain their own original claims.
#[derive(Default)]
struct Retired {
    _open: Option<Open>,
    _device: Option<Arc<GpuDevice>>,
    #[cfg(all(windows, feature = "native-composition"))]
    _host_graphics: Option<Arc<sugarsubstitute_desktop::canvas::CanvasGraphics>>,
    _gpu_front: Option<GpuSurface>,
    _presentation: Option<WindowFrame>,
    _drawing: Option<work::Draw>,
    _projection: Option<Box<CpuProjection>>,
    _buffers: [Option<ProjectionBuffer>; 2],
    _scene: Option<Scene>,
    _document: Option<document::DocumentState>,
    _packet: Option<Packet>,
    _resources: Option<Resources>,
    _front: Option<Arc<CpuSurface>>,
    _fallback_front: Option<Arc<CpuSurface>>,
    _fallback_gpu_front: Option<GpuSurface>,
    _navigation_front: Option<Arc<CpuSurface>>,
    _navigation_gpu_front: Option<GpuSurface>,
    _proof_gpu: Option<sugarsubstitute_canvas_render::GpuFrame>,
    _update: Option<Box<sugarsubstitute_canvas_render::PreparedUpdate>>,
    #[cfg(test)]
    _fixture_memory: Option<MemoryLease>,
}
/// The original admitted request and its quality remain paired through cancellation acknowledgement.
struct Rendering {
    job: OwnedJob<Packet, WorkError>,
    reconstruction: Reconstruction,
    tier: BackingTier,
    coverage: BackingCoverage,
    display_extent: sugarsubstitute_image_ops::Extent,
    superseded: bool,
}
struct Loading {
    intent: u64,
    expected: Option<AssetId>,
    job: OwnedJob<Scene, WorkError>,
    #[cfg(feature = "native-composition")]
    acquirer: Option<Arc<dyn acquisition::SourceAcquirer>>,
}
#[derive(Clone, Copy)]
struct Presented {
    sampling_scale: f64,
    tier: BackingTier,
    coverage: BackingCoverage,
    view: Viewport,
    reconstruction: Reconstruction,
    document: (u64, u64),
    source_revision: u64,
}
/// Native callbacks mutate only application intent and retained presentation, never image kernels.
pub(crate) struct Session {
    config: Config,
    labels: &'static ViewerMessages,
    title: String,
    health: DiagnosticHealth,
    clock: Arc<dyn Clock>,
    sequence: JobSequence,
    runtime: Option<Runtime>,
    starting_owner: Option<ExecutionOwner>,
    initializing: Option<OwnedJob<(Resources, Scene), WorkError>>,
    device_job: Option<OwnedJob<Arc<GpuDevice>, WorkError>>,
    #[cfg(all(windows, feature = "native-composition"))]
    host_graphics_job:
        Option<OwnedJob<Arc<sugarsubstitute_desktop::canvas::CanvasGraphics>, WorkError>>,
    #[cfg(all(windows, feature = "native-composition"))]
    host_graphics_pending: bool,
    #[cfg(all(windows, feature = "native-composition"))]
    host_graphics: Option<Arc<sugarsubstitute_desktop::canvas::CanvasGraphics>>,
    #[cfg(all(windows, feature = "native-composition"))]
    host_graphics_failure: Option<Box<dyn Error>>,
    device_pending: bool,
    gpu_ready: bool,
    gpu_descriptor: Option<AdapterDescriptor>,
    gpu_diagnostics: Option<sugarsubstitute_image_gpu::GpuDiagnostics>,
    presentation: attachment::Presentation,
    resources: Option<Resources>,
    document: document::DocumentState,
    loading: Option<Loading>,
    #[cfg(feature = "native-composition")]
    acquisition_retirement: Option<acquisition::Retirement>,
    pending_open: Option<Open>,
    /// Latest unresolved source identity and input clock before any image reaches native custody.
    pending_first_image: Option<(u64, Duration)>,
    /// Intent-through-native-queue acceptance, including import/load but excluding scanout.
    first_image_queue_latency: Option<Duration>,
    selection: u64,
    rendering: Option<Rendering>,
    displaying: Option<display::Displaying>,
    pending_retirement: Option<Retired>,
    deferred_retirement: Option<Retired>,
    retirement: Option<OwnedJob<(), WorkError>>,
    retirement_deadline: Option<Moment>,
    deferred_retirement_deadline: Option<Moment>,
    gpu_presentation: Option<GpuPresentation>,
    pending_gpu_presentation: Option<presentation::PendingPresentation>,
    presenting: Option<presentation::Presenting>,
    awaiting_gpu: Option<FrameStamp>,
    coverage_acceptances: u64,
    fallback_projection_acceptances: u64,
    navigation_projection_acceptances: u64,
    bootstrap_projection_acceptances: u64,
    first_fallback_preparation_io: Option<ArchiveStats>,
    pacing: PresentationPacer,
    input_observed: Duration,
    expose_pending: bool,
    drawable: bool,
    frame_revision: u64,
    picker: bool,
    failed: Option<Box<dyn Error>>,
    closing: bool,
    shutdown_deadline: Option<Moment>,
    shutdown_expiry_reported: bool,
    shutdown_started: bool,
    initialization_pending: bool,
    pub(super) last_prepare: Duration,
    pub(super) last_stats: sugarsubstitute_image_ops::EvaluationStats,
    pub(super) last_surface_work: sugarsubstitute_canvas_render::SurfaceWork,
}
impl Session {
    /// Report completed storage and document initialization without claiming a visible frame.
    pub(crate) fn initialized(&self) -> bool {
        self.resources.is_some()
            && self.document.scene.is_some()
            && self.failed.is_none()
            && !self.closing
    }
    /// One elapsed clock epoch supplies navigation and execution deadlines.
    fn navigation_time(&self) -> Duration {
        Duration::from_nanos(self.clock.now().as_nanos())
    }
    /// Create inert state with the composing execution lifetime's clock and job sequence.
    /// Execution begins only when the desktop supplies its wake.
    pub(crate) fn new(
        config: Config,
        health: DiagnosticHealth,
        clock: Arc<dyn Clock>,
        sequence: JobSequence,
    ) -> Result<Self, Box<dyn Error>> {
        let labels = messages(&config.locale).viewer;
        let view = Viewport::new(config.extent)?;
        let gpu_requested = config.gpu.is_some();
        Ok(Self {
            config,
            labels,
            title: labels.empty.to_owned(),
            health,
            clock,
            sequence,
            runtime: None,
            starting_owner: None,
            initializing: None,
            device_job: None,
            #[cfg(all(windows, feature = "native-composition"))]
            host_graphics_job: None,
            #[cfg(all(windows, feature = "native-composition"))]
            host_graphics_pending: false,
            #[cfg(all(windows, feature = "native-composition"))]
            host_graphics: None,
            #[cfg(all(windows, feature = "native-composition"))]
            host_graphics_failure: None,
            device_pending: gpu_requested,
            gpu_ready: !gpu_requested,
            gpu_descriptor: None,
            gpu_diagnostics: None,
            presentation: attachment::Presentation::Headless,
            resources: None,
            document: document::DocumentState::new(view),
            loading: None,
            #[cfg(feature = "native-composition")]
            acquisition_retirement: None,
            pending_open: None,
            pending_first_image: None,
            first_image_queue_latency: None,
            selection: 1,
            rendering: None,
            displaying: None,
            pending_retirement: None,
            deferred_retirement: None,
            retirement: None,
            retirement_deadline: None,
            deferred_retirement_deadline: None,
            gpu_presentation: None,
            pending_gpu_presentation: None,
            presenting: None,
            awaiting_gpu: None,
            coverage_acceptances: 0,
            fallback_projection_acceptances: 0,
            navigation_projection_acceptances: 0,
            bootstrap_projection_acceptances: 0,
            first_fallback_preparation_io: None,
            pacing: PresentationPacer::new(Duration::from_nanos(16_666_667))?,
            input_observed: Duration::ZERO,
            expose_pending: false,
            drawable: true,
            frame_revision: 0,
            picker: false,
            failed: None,
            closing: false,
            shutdown_deadline: None,
            shutdown_expiry_reported: false,
            shutdown_started: false,
            initialization_pending: false,
            last_prepare: Duration::ZERO,
            last_stats: Default::default(),
            last_surface_work: Default::default(),
        })
    }
    /// Bind work to one original deadline and the same sole job identity sequence.
    fn spec(
        &self,
        operation: Operation,
        class: JobClass,
        bytes: u64,
        duration: Duration,
        replacement: Option<ReplacementKey>,
    ) -> Result<JobSpec, Box<dyn Error>> {
        Ok(JobSpec {
            id: self.sequence.allocate()?,
            operation,
            class,
            deadline: Some(self.clock.now().checked_add(duration)?),
            reservation_bytes: bytes,
            replacement,
        })
    }
    /// Adopt one original error without destroying the currently displayed image.
    fn fail(&mut self, cause: impl Into<Box<dyn Error>>) {
        let cause = cause.into();
        let mut current = Some(cause.as_ref());
        while let Some(error) = current {
            if let Some(memory) = error.downcast_ref::<MemoryClaimError>() {
                tracing::error!(
                    operation = "canvas_viewer_memory",
                    requested_bytes = memory.requested_bytes,
                    available_bytes = memory.available_bytes,
                    "Canvas allocation exceeded its original execution admission"
                );
                break;
            }
            current = error.source();
        }
        tracing::error!(operation="canvas_viewer_work", cause=%cause, "Canvas operation failed");
        if self.failed.is_none() {
            self.failed = Some(cause);
        }
        self.title = self.labels.failed.to_owned();
    }
    /// Consume one terminal delivery and its exact identity; resource claims travel inside values.
    fn take<T>(slot: &mut Option<OwnedJob<T, WorkError>>) -> Option<JobOutcome<T, WorkError>> {
        let job = slot.as_mut()?;
        let completion = job.try_take_completion()?;
        Self::observe_terminal(job, &completion.outcome);
        drop(slot.take());
        Some(completion.outcome)
    }
    /// Preserve bounded failure category and execution identity while private causes stay owned.
    fn observe_terminal<T>(job: &OwnedJob<T, WorkError>, outcome: &JobOutcome<T, WorkError>) {
        if matches!(
            outcome,
            JobOutcome::Failure(_) | JobOutcome::Panicked(_) | JobOutcome::TimedOut
        ) {
            let spec = job.spec();
            tracing::warn!(
                operation = "canvas_execution_terminal",
                job_id = spec.id.get(),
                work_kind = ?spec.operation,
                capacity_class = ?spec.class,
                terminal_kind = ?outcome.kind(),
                "Canvas execution reached an unsuccessful terminal outcome"
            );
        }
    }
    /// Preserve supervision failure categories while keeping arbitrary panic text out of logs.
    fn terminal<T>(outcome: JobOutcome<T, WorkError>) -> Result<T, Box<dyn Error>> {
        match outcome {
            JobOutcome::Success(value) => Ok(value),
            JobOutcome::Failure(cause) => Err(cause.into()),
            JobOutcome::Panicked(cause) => Err(Box::new(WorkerPanic { _cause: cause })),
            JobOutcome::Cancelled => Err("Canvas work stopped after cancellation".into()),
            JobOutcome::TimedOut => Err("Canvas work stopped after its original deadline".into()),
            JobOutcome::Coalesced { .. } => {
                Err("Canvas work was replaced before preparation".into())
            }
        }
    }
}
#[derive(Debug)]
struct WorkerPanic {
    _cause: PanicCause,
}
impl std::fmt::Display for WorkerPanic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Canvas worker panicked")
    }
}
impl Error for WorkerPanic {}
