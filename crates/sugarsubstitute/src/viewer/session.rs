// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate one camera/document lifetime with typed shared-execution jobs and worker retirement.
mod activity;
mod completion;
mod gpu;
mod native;
mod proof;
#[cfg(test)]
mod tests;
use super::{
    options::Options,
    work::{self, Packet, Resources, Scene},
    work_error::WorkError,
};
use crate::{diagnostics::DiagnosticHealth, job_sequence::JobSequence};
use std::{error::Error, path::PathBuf, sync::Arc, time::Duration};
use sugarsubstitute_canvas::{DamagePlan, FrameStamp, Viewport};
use sugarsubstitute_canvas_render::{CpuSurface, GpuSurface};
use sugarsubstitute_contracts::{
    execution::*,
    localization::{messages, viewer::ViewerMessages},
};
use sugarsubstitute_desktop::canvas::WindowTarget;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock};
use sugarsubstitute_image_gpu::{AdapterDescriptor, GpuDevice, WindowFrame};
use sugarsubstitute_image_io::CacheReuse;
use sugarsubstitute_image_ops::{AssetId, Filter};

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
    _scene: Option<Scene>,
    _packet: Option<Packet>,
    _resources: Option<Resources>,
    _front: Option<CpuSurface>,
    _update: Option<Box<sugarsubstitute_canvas_render::PreparedUpdate>>,
}
/// The original admitted request and its quality remain paired through cancellation acknowledgement.
struct Rendering {
    job: OwnedJob<Packet, WorkError>,
    filter: Filter,
    superseded: bool,
}
struct Loading {
    intent: u64,
    expected: Option<AssetId>,
    job: OwnedJob<Scene, WorkError>,
}
struct Presented {
    view: Viewport,
    filter: Filter,
    document: (u64, u64),
    source_revision: u64,
    damage: DamagePlan,
    base: Option<FrameStamp>,
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
    native_target: Option<WindowTarget>,
    resources: Option<Resources>,
    scene: Option<Scene>,
    loading: Option<Loading>,
    pending_open: Option<Open>,
    selection: u64,
    rendering: Option<Rendering>,
    pending_retirement: Option<Retired>,
    retirement: Option<OwnedJob<(), WorkError>>,
    retirement_deadline: Option<Moment>,
    view: Viewport,
    navigation_until: Option<Moment>,
    initial_preview: bool,
    front: Option<CpuSurface>,
    gpu_front: Option<GpuSurface>,
    gpu_presentation: Option<WindowFrame>,
    awaiting_gpu: Option<FrameStamp>,
    native_presented: Option<FrameStamp>,
    native_verified: bool,
    expose_pending: bool,
    presented: Option<Presented>,
    frame_revision: u64,
    pointer: [f64; 2],
    dragging: bool,
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
    /// Preview only during the current navigation interval; settled output always refines.
    fn filter(&self) -> Filter {
        if self.initial_preview
            || self.navigation_until.is_some_and(|deadline| self.clock.now() < deadline)
        {
            Filter::Nearest
        } else {
            Filter::Lanczos3
        }
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
            native_target: None,
            resources: None,
            scene: None,
            loading: None,
            pending_open: None,
            selection: 1,
            rendering: None,
            pending_retirement: None,
            retirement: None,
            retirement_deadline: None,
            view,
            navigation_until: None,
            initial_preview: false,
            front: None,
            gpu_front: None,
            gpu_presentation: None,
            awaiting_gpu: None,
            native_presented: None,
            native_verified: false,
            expose_pending: false,
            presented: None,
            frame_revision: 0,
            pointer: [0.0; 2],
            dragging: false,
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
        self.selection = self.selection.checked_add(1).ok_or("Canvas source revision exhausted")?;
        self.pending_open = Some(Open { path, intent: self.selection, reuse, expected });
        if let Some(loading) = &self.loading {
            loading.job.cancel();
        }
        if let Some(rendering) = &self.rendering {
            rendering.job.cancel();
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
        let completion = slot.as_mut()?.try_take_completion()?;
        drop(slot.take());
        Some(completion.outcome)
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
