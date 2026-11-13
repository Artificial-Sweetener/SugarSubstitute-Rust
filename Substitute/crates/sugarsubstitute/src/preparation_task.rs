// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt admitted splash initialization and raster tasks to one typed host result.

use crate::render_transfer::{PreparationTransfer, TransferError};
use std::{error::Error, fmt};
use sugarsubstitute_application::transcript::TranscriptSnapshot;
use sugarsubstitute_contracts::{
    execution::{CpuStep, CpuTask, JobContext, MemoryLease},
    presentation::{FrameIdentity, PreparedFrame},
};
use sugarsubstitute_splash::{
    PreparationError, RenderError, SplashInitializationTask, SplashRenderRequest, SplashRenderTask,
    SplashRenderer,
};

/// Retain either an application's exclusive transcript or a complete charged output view.
pub(crate) enum RenderConsole {
    Owned(TranscriptSnapshot),
    Output(crate::OutputSnapshot),
}
impl sugarsubstitute_splash::TranscriptView for RenderConsole {
    fn transcript(&self) -> &TranscriptSnapshot {
        match self {
            Self::Owned(view) => view,
            Self::Output(view) => view.transcript(),
        }
    }
}
impl std::ops::Deref for RenderConsole {
    type Target = TranscriptSnapshot;
    fn deref(&self) -> &Self::Target {
        sugarsubstitute_splash::TranscriptView::transcript(self)
    }
}
impl From<TranscriptSnapshot> for RenderConsole {
    fn from(view: TranscriptSnapshot) -> Self {
        Self::Owned(view)
    }
}
impl From<crate::OutputSnapshot> for RenderConsole {
    fn from(view: crate::OutputSnapshot) -> Self {
        Self::Output(view)
    }
}

/// Retain the actual non-clonable request across rejection or an uninvoked accepted factory.
pub(crate) struct PendingPreparation {
    pub(crate) identity: FrameIdentity,
    pub(crate) appearance: sugarsubstitute_splash::Appearance,
    pub(crate) bytes: u64,
    pub(crate) input: PreparationTransfer<SplashRenderRequest<RenderConsole>>,
}
impl PendingPreparation {
    pub(crate) fn new(request: SplashRenderRequest<RenderConsole>) -> Self {
        Self {
            identity: request.identity,
            appearance: request.appearance,
            bytes: request.reservation_bytes(),
            input: PreparationTransfer::new(request),
        }
    }
}

/// Return cache ownership with every ordinarily completed raster operation.
pub(crate) struct PreparationOutput {
    pub(crate) renderer: SplashRenderer,
    pub(crate) frame: PreparationFrame,
    pub(crate) console: Option<RenderConsole>,
    pub(crate) console_clipped_rows: u8,
}
/// Distinguish initialization from an ordinarily completed frame without duplicating owned storage.
pub(crate) enum PreparationFrame {
    /// Initialization completed without decoding unrequested poses or discovering fonts.
    Initialized,
    /// Raster work completed, retaining the renderer even when ordinary rendering failed.
    Prepared(Result<PreparedFrame, RenderError>),
}

/// Preserve original task and transfer failures at the composed execution boundary.
#[derive(Debug)]
pub(crate) enum PreparationFailure {
    /// Packaged inventory or admitted cache initialization failed.
    Render(RenderError),
    /// Resumable preparation violated its task-state contract.
    Task(PreparationError),
    /// An accepted factory could not recover its exclusively owned cache.
    Transfer(TransferError),
}

impl fmt::Display for PreparationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Render(error) => error.fmt(formatter),
            Self::Task(error) => error.fmt(formatter),
            Self::Transfer(error) => error.fmt(formatter),
        }
    }
}
impl Error for PreparationFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Render(error) => error,
            Self::Task(error) => error,
            Self::Transfer(error) => error,
        })
    }
}

/// Initialize admitted cache metadata without allocating a boxed raster task.
pub(crate) struct InitializationPreparation(SplashInitializationTask);

impl InitializationPreparation {
    /// Construct only after atomic admission on the CPU worker.
    pub(crate) fn new(locale: String, memory: MemoryLease) -> Self {
        Self(SplashRenderer::initialize(locale, memory))
    }
}

impl CpuTask<PreparationOutput, PreparationFailure> for InitializationPreparation {
    fn step(
        &mut self,
        context: &JobContext,
    ) -> Result<CpuStep<PreparationOutput>, PreparationFailure> {
        match self.0.step(context).map_err(PreparationFailure::Task)? {
            CpuStep::Yield => Ok(CpuStep::Yield),
            CpuStep::Complete(renderer) => renderer
                .map(|renderer| PreparationOutput {
                    renderer,
                    frame: PreparationFrame::Initialized,
                    console: None,
                    console_clipped_rows: 0,
                })
                .map(CpuStep::Complete)
                .map_err(PreparationFailure::Render),
        }
    }
}

/// Own the admitted raster cursor or its original cache transfer failure.
pub(crate) struct FramePreparation {
    task: Result<SplashRenderTask<RenderConsole>, TransferError>,
    // Retain actual input through CPU destruction when cache acquisition failed.
    _failed_console: Option<RenderConsole>,
}

impl FramePreparation {
    /// Take cache ownership on the worker, never during UI-thread submission.
    pub(crate) fn new(
        transfer: PreparationTransfer<SplashRenderer>,
        request: PreparationTransfer<SplashRenderRequest<RenderConsole>>,
        bootstrap: bool,
        memory: MemoryLease,
    ) -> Self {
        let mut request = match request.take() {
            Ok(request) => request,
            Err(cause) => return Self { task: Err(cause), _failed_console: None },
        };
        request.bootstrap = bootstrap;
        match transfer.take() {
            Ok(renderer) => {
                Self { task: Ok(renderer.prepare(request, memory)), _failed_console: None }
            }
            Err(cause) => Self { task: Err(cause), _failed_console: request.console },
        }
    }
}

impl CpuTask<PreparationOutput, PreparationFailure> for FramePreparation {
    fn step(
        &mut self,
        context: &JobContext,
    ) -> Result<CpuStep<PreparationOutput>, PreparationFailure> {
        match &mut self.task {
            Ok(task) => match task.step(context).map_err(PreparationFailure::Task)? {
                CpuStep::Yield => Ok(CpuStep::Yield),
                CpuStep::Complete(result) => Ok(CpuStep::Complete(PreparationOutput {
                    renderer: result.renderer,
                    frame: PreparationFrame::Prepared(result.frame),
                    console: result.console,
                    console_clipped_rows: result.console_clipped_rows,
                })),
            },
            Err(error) => Err(PreparationFailure::Transfer(match error {
                TransferError::Consumed => TransferError::Consumed,
                TransferError::Poisoned => TransferError::Poisoned,
            })),
        }
    }
}
