// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adapt admitted splash initialization and raster tasks to one typed host result.

use crate::render_transfer::{RendererTransfer, TransferError};
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{
    execution::{CpuStep, CpuTask, JobContext, MemoryLease},
    presentation::PreparedFrame,
};
use sugarsubstitute_splash::{
    PreparationError, RenderError, SplashInitializationTask, SplashRenderRequest, SplashRenderTask,
    SplashRenderer,
};

/// Return cache ownership with every ordinarily completed raster operation.
pub(crate) enum PreparationOutput {
    /// Initialization completed without decoding unrequested poses or discovering fonts.
    Initialized(SplashRenderer),
    /// Raster work completed, retaining the renderer even when ordinary rendering failed.
    Prepared { renderer: SplashRenderer, frame: Result<PreparedFrame, RenderError> },
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
                .map(PreparationOutput::Initialized)
                .map(CpuStep::Complete)
                .map_err(PreparationFailure::Render),
        }
    }
}

/// Own the admitted raster cursor or its original cache transfer failure.
pub(crate) struct FramePreparation(Result<SplashRenderTask, TransferError>);

impl FramePreparation {
    /// Take cache ownership on the worker, never during UI-thread submission.
    pub(crate) fn new(
        transfer: RendererTransfer<SplashRenderer>,
        request: SplashRenderRequest,
        memory: MemoryLease,
    ) -> Self {
        Self(transfer.take().map(|renderer| renderer.prepare(request, memory)))
    }
}

impl CpuTask<PreparationOutput, PreparationFailure> for FramePreparation {
    fn step(
        &mut self,
        context: &JobContext,
    ) -> Result<CpuStep<PreparationOutput>, PreparationFailure> {
        match &mut self.0 {
            Ok(task) => match task.step(context).map_err(PreparationFailure::Task)? {
                CpuStep::Yield => Ok(CpuStep::Yield),
                CpuStep::Complete(result) => Ok(CpuStep::Complete(PreparationOutput::Prepared {
                    renderer: result.renderer,
                    frame: result.frame,
                })),
            },
            Err(error) => Err(PreparationFailure::Transfer(match error {
                TransferError::Consumed => TransferError::Consumed,
                TransferError::Poisoned => TransferError::Poisoned,
            })),
        }
    }
}
