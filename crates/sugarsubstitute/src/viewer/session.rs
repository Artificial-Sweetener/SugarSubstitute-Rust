// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate one camera/document lifetime with typed shared-execution jobs and worker retirement.
mod activity;
mod backing;
mod completion;
mod display;
mod gpu;
mod native;
mod preparation;
mod presentation;
mod proof;
mod startup;
#[cfg(test)]
mod tests;
use self::display::DisplayEvidence;
use super::{
    coverage::{self, BackingTier},
    options::Options,
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
use sugarsubstitute_desktop::canvas::{GpuPresentation, WindowTarget};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock};
use sugarsubstitute_image_gpu::{AdapterDescriptor, GpuDevice, WindowFrame};
use sugarsubstitute_image_io::{ArchiveStats, CacheReuse};
use sugarsubstitute_image_ops::{AssetId, Reconstruction};

/// Actual executor/parent scope remain retained until all typed jobs and worker notifications stop.
struct Runtime {
    owner: ExecutionOwner,
    client: ExecutionClient,
    scope: Scope,
}
/// Latest explicit open intent; replacing it never turns an earlier completion into current content.
struct Open {
    path: PathBuf,
    intent: u64,
    reuse: CacheReuse,
    expected: Option<AssetId>,
}
/// One bounded admitted disposal transaction; payload fields retain their own original claims.
#[derive(Default)]
struct Retired {
    _device: Option<Arc<GpuDevice>>,
    _gpu_front: Option<GpuSurface>,
    _presentation: Option<WindowFrame>,
    _drawing: Option<work::Draw>,
    _projection: Option<Box<CpuProjection>>,
    _buffers: [Option<ProjectionBuffer>; 2],
    _scene: Option<Scene>,
    _packet: Option<Packet>,
    _resources: Option<Resources>,
    _front: Option<Arc<CpuSurface>>,
    _fallback_front: Option<Arc<CpuSurface>>,
    _fallback_gpu_front: Option<GpuSurface>,
    _navigation_front: Option<Arc<CpuSurface>>,
    _navigation_gpu_front: Option<GpuSurface>,
    _proof_gpu: Option<sugarsubstitute_canvas_render::GpuFrame>,
    _update: Option<Box<sugarsubstitute_canvas_render::PreparedUpdate>>,
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
}
#[derive(Clone, Copy)]
struct Presented {
    tier: BackingTier,
    coverage: BackingCoverage,
    view: Viewport,
    reconstruction: Reconstruction,
    document: (u64, u64),
    source_revision: u64,
}
/// Native callbacks mutate only application intent and retained presentation, never image kernels.
pub(super) struct Session {
    options: Options,
    labels: &'static ViewerMessages,
    title: String,
    health: DiagnosticHealth,
    clock: Arc<dyn Clock>,
    sequence: JobSequence,
    runtime: Option<Runtime>,
    initializing: Option<OwnedJob<(Resources, Scene), WorkError>>,
    device_job: Option<OwnedJob<Arc<GpuDevice>, WorkError>>,
    device_pending: bool,
    gpu_ready: bool,
    gpu_descriptor: Option<AdapterDescriptor>,
    gpu_diagnostics: Option<sugarsubstitute_image_gpu::GpuDiagnostics>,
    native_target: Option<WindowTarget>,
    resources: Option<Resources>,
    scene: Option<Scene>,
    loading: Option<Loading>,
    pending_open: Option<Open>,
    /// Latest unresolved source identity and input clock before any image reaches native custody.
    pending_first_image: Option<(u64, Duration)>,
    /// Intent-through-native-queue acceptance, including import/load but excluding scanout.
    first_image_queue_latency: Option<Duration>,
    selection: u64,
    rendering: Option<Rendering>,
    displaying: Option<display::Displaying>,
    pending_retirement: Option<Retired>,
    retirement: Option<OwnedJob<(), WorkError>>,
    retirement_deadline: Option<Moment>,
    tools: ToolManager,
    initial_preview: bool,
    front: Option<Arc<CpuSurface>>,
    fallback_front: Option<Arc<CpuSurface>>,
    navigation_front: Option<Arc<CpuSurface>>,
    gpu_front: Option<GpuSurface>,
    fallback_gpu_front: Option<GpuSurface>,
    navigation_gpu_front: Option<GpuSurface>,
    gpu_presentation: Option<GpuPresentation>,
    pending_gpu_presentation: Option<presentation::PendingPresentation>,
    presenting: Option<presentation::Presenting>,
    awaiting_gpu: Option<FrameStamp>,
    native_presented: Option<FrameStamp>,
    native_verified: bool,
    display_source: Option<FrameStamp>,
    display_view: Option<Viewport>,
    display_evidence: Option<DisplayEvidence>,
    coverage_acceptances: u64,
    fallback_projection_acceptances: u64,
    navigation_projection_acceptances: u64,
    first_fallback_preparation_io: Option<ArchiveStats>,
    pending_cpu_evidence: Option<DisplayEvidence>,
    pending_gpu_evidence: Option<DisplayEvidence>,
    pacing: PresentationPacer,
    input_observed: Duration,
    cpu_pending: Option<FrameStamp>,
    cpu_projection: Option<Box<CpuProjection>>,
    projection_damage: Option<DamagePlan>,
    projection_base: Option<FrameStamp>,
    display_buffers: [Option<ProjectionBuffer>; 2],
    display_buffer_extent: Option<sugarsubstitute_image_ops::Extent>,
    expose_pending: bool,
    drawable: bool,
    presented: Option<Presented>,
    fallback_presented: Option<Presented>,
    navigation_presented: Option<Presented>,
    frame_revision: u64,
    picker: bool,
    failed: Option<Box<dyn Error>>,
    closing: bool,
    shutdown_started: bool,
    initialization_pending: bool,
    rebuilt: bool,
    pub(super) last_prepare: Duration,
    pub(super) last_stats: sugarsubstitute_image_ops::EvaluationStats,
    pub(super) last_surface_work: sugarsubstitute_canvas_render::SurfaceWork,
}
impl Session {
    /// One elapsed clock epoch supplies navigation and execution deadlines.
    fn navigation_time(&self) -> Duration {
        Duration::from_nanos(self.clock.now().as_nanos())
    }
    /// Create inert native application state; execution begins only when the desktop supplies wake.
    pub(super) fn new(options: Options, health: DiagnosticHealth) -> Result<Self, Box<dyn Error>> {
        let labels = messages(&options.locale).viewer;
        let view = Viewport::new(options.extent)?;
        let gpu_requested = options.gpu.is_some();
        Ok(Self {
            options,
            labels,
            title: labels.empty.to_owned(),
            health,
            clock: MonotonicClock::new(),
            sequence: JobSequence::new(),
            runtime: None,
            initializing: None,
            device_job: None,
            device_pending: gpu_requested,
            gpu_ready: !gpu_requested,
            gpu_descriptor: None,
            gpu_diagnostics: None,
            native_target: None,
            resources: None,
            scene: None,
            loading: None,
            pending_open: None,
            pending_first_image: None,
            first_image_queue_latency: None,
            selection: 1,
            rendering: None,
            displaying: None,
            pending_retirement: None,
            retirement: None,
            retirement_deadline: None,
            tools: ToolManager::new(Navigation::new(view)),
            initial_preview: false,
            front: None,
            fallback_front: None,
            navigation_front: None,
            gpu_front: None,
            fallback_gpu_front: None,
            navigation_gpu_front: None,
            gpu_presentation: None,
            pending_gpu_presentation: None,
            presenting: None,
            awaiting_gpu: None,
            native_presented: None,
            native_verified: false,
            display_source: None,
            display_view: None,
            display_evidence: None,
            coverage_acceptances: 0,
            fallback_projection_acceptances: 0,
            navigation_projection_acceptances: 0,
            first_fallback_preparation_io: None,
            pending_cpu_evidence: None,
            pending_gpu_evidence: None,
            pacing: PresentationPacer::new(Duration::from_nanos(16_666_667))?,
            input_observed: Duration::ZERO,
            cpu_pending: None,
            cpu_projection: None,
            projection_damage: None,
            projection_base: None,
            display_buffers: [None, None],
            display_buffer_extent: None,
            expose_pending: false,
            drawable: true,
            presented: None,
            fallback_presented: None,
            navigation_presented: None,
            frame_revision: 0,
            picker: false,
            failed: None,
            closing: false,
            shutdown_started: false,
            rebuilt: false,
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
    /// Record a source selection before cancelling old work; the old document remains retained.
    fn open(
        &mut self,
        path: PathBuf,
        reuse: CacheReuse,
        expected: Option<AssetId>,
    ) -> Result<(), Box<dyn Error>> {
        self.input_observed = self.navigation_time();
        self.selection = self.selection.checked_add(1).ok_or("Canvas source revision exhausted")?;
        if self.first_image_queue_latency.is_none() {
            self.pending_first_image = Some((self.selection, self.input_observed));
        }
        self.pending_open = Some(Open { path, intent: self.selection, reuse, expected });
        if let Some(loading) = &self.loading {
            loading.job.cancel();
        }
        if let Some(rendering) = &self.rendering {
            rendering.job.cancel();
        }
        if let Some(presenting) = &self.presenting {
            presenting.job.cancel();
        }
        self.failed = None;
        self.title = self.labels.opening.to_owned();
        Ok(())
    }
    /// Retain rejected cleanup input unchanged until the authoritative execution adapter accepts it.
    fn retire(&mut self, value: Retired) -> Result<(), Box<dyn Error>> {
        if self.pending_retirement.is_some() {
            return Err("Canvas retirement input is already retained".into());
        }
        self.retirement_deadline = Some(self.clock.now().checked_add(Duration::from_secs(5))?);
        self.pending_retirement = Some(value);
        Ok(())
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
