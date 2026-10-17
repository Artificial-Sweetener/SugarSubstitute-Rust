// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Move an exclusive renderer through admitted initialization and bounded frame preparation.

use super::{RenderError, RenderFrame, SplashRenderer, stages::RenderStages};
use crate::{AnimationFrame, Appearance, ProgressFrame};
use std::{error::Error, fmt, num::NonZeroUsize};
use sugarsubstitute_application::transcript::TranscriptSnapshot;
use sugarsubstitute_contracts::{
    execution::{CpuStep, CpuTask, JobContext, MemoryClaim, MemoryLease},
    presentation::{FrameIdentity, FrameShape, FrameValidator, PreparedFrame},
};

/// Reserve all forty decoded poses and both current/replacement caption pairs.
///
/// Pose pixels occupy 23839360 bytes; caption replacement pairs occupy <=376832.
/// Metadata fits the remaining allowance. Opaque native font allocations are measured separately.
pub const SPLASH_CACHE_RESERVATION_BYTES: u64 = 24 * 1024 * 1024;
const GEOMETRY_SCRATCH: u64 = 1024 * 1024;
const DECODER_AND_GLYPH_SCRATCH: u64 = 6 * 1024 * 1024;

/// Borrow immutable transcript data while its complete external custody stays in the task.
/// The implementation must keep the same snapshot alive through result disposal.
pub trait TranscriptView: Send + 'static {
    /// Borrow the bounded transcript without extracting its shared allocations.
    fn transcript(&self) -> &TranscriptSnapshot;
}
impl TranscriptView for TranscriptSnapshot {
    fn transcript(&self) -> &TranscriptSnapshot {
        self
    }
}

/// Capture one immutable view revision for a retained exclusive renderer.
pub struct SplashRenderRequest<T = TranscriptSnapshot> {
    /// Validated tightly packed device-pixel shape; splash scale must additionally be >=0.5.
    pub shape: FrameShape,
    /// Surface/state generation and sequence for the host's stale-result checks.
    pub identity: FrameIdentity,
    /// Native material, disclosure and control state snapshot.
    pub appearance: Appearance,
    /// Pose, direction and animation geometry snapshot.
    pub character: AnimationFrame,
    /// Producer-owned completion and activity snapshot.
    pub feedback: ProgressFrame,
    /// Defer status font preparation for the first frame.
    pub bootstrap: bool,
    /// Actual immutable transcript pin, retained through validation and result disposal.
    /// Root verifies its resource against the current managed resource; presentation grants no authority.
    pub console: Option<T>,
    /// Sole text presentation owner, including before the first source record arrives.
    /// Native mode reserves its independent bounded document beside the two pixel copies.
    pub console_presentation: sugarsubstitute_contracts::presentation::ConsolePresentation,
}
impl<T> SplashRenderRequest<T> {
    /// Bound output plus live scratch and the host's later RGBA/BGRA retained pair.
    ///
    /// At most two one-byte masks coexist, with fixed geometry overhead. PNG scratch
    /// and bounded native glyph buffers reuse the six-MiB stage allowance. During
    /// validator handoff the same output is conservatively charged twice until its
    /// previous claim is dropped; no pixel copy occurs. Allocation capacity is checked.
    pub fn reservation_bytes(&self) -> u64 {
        let output = self.shape.byte_len() as u64;
        (2 * output).max(output + self.scratch_bytes()) + self.console_presentation.storage_bytes()
    }
    /// Bound stage scratch independently from decoded cache ownership.
    fn scratch_bytes(&self) -> u64 {
        (self.shape.byte_len() as u64 / 2 + GEOMETRY_SCRATCH).max(DECODER_AND_GLYPH_SCRATCH)
    }
    fn frame(&self) -> RenderFrame {
        RenderFrame {
            width: self.shape.width(),
            height: self.shape.height(),
            scale: self.shape.scale(),
            appearance: self.appearance,
            character: self.character,
            feedback: self.feedback,
            status: !self.bootstrap,
        }
    }
}
/// Return ordinary render failure with the same cache-owning renderer for the next revision.
pub struct SplashPreparationResult<T = TranscriptSnapshot> {
    /// Exclusive owner recovered after actual job completion, including ordinary failures.
    pub renderer: SplashRenderer,
    /// Immutable prepared pixels or the original typed rendering error.
    pub frame: Result<PreparedFrame, RenderError>,
    /// Return the same original snapshot after task scratch/validation disposal.
    /// Keep it until the observed terminal result is published, rejected or discarded.
    pub console: Option<T>,
    /// Actual rows clipped by projection bounds, from zero through ten.
    /// Bootstrap and absent-console requests report zero; full source text remains pinned.
    pub console_clipped_rows: u8,
}
/// Reject repeated use or a broken private task-state invariant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationError {
    /// The executor called a completed task again or its owned state is absent.
    InvalidTaskState,
}
impl fmt::Display for PreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Invalid splash preparation task state")
    }
}
impl Error for PreparationError {}

/// Own lazy admitted renderer initialization without decoding a pose or discovering fonts.
pub struct SplashInitializationTask {
    input: Option<(String, MemoryLease)>,
}
impl SplashInitializationTask {
    pub(super) fn new(locale: String, lease: MemoryLease) -> Self {
        Self { input: Some((locale, lease)) }
    }
}
impl CpuTask<Result<SplashRenderer, RenderError>, PreparationError> for SplashInitializationTask {
    /// Validate at most forty metadata entries and claim cache memory in the first admitted step.
    fn step(
        &mut self,
        _context: &JobContext,
    ) -> Result<CpuStep<Result<SplashRenderer, RenderError>>, PreparationError> {
        let (locale, lease) = self.input.take().ok_or(PreparationError::InvalidTaskState)?;
        Ok(CpuStep::Complete(SplashRenderer::new_admitted(&locale, lease)))
    }
}

/// Retain bounded stage cursors and allocation claims through cooperative execution yields.
///
/// No UI operation, worker creation or background runtime occurs here. The execution
/// adapter observes cancellation/deadlines between steps and owns actual task destruction.
/// Renderer native resources are free-threaded but drawing is exclusively serialized.
pub struct SplashRenderTask<T = TranscriptSnapshot> {
    lease: Option<MemoryLease>,
    raster: Option<RenderStages>,
    validator: Option<FrameValidator>,
    // Pending decoded cache pixels are destroyed before the renderer's cache claim.
    renderer: Option<SplashRenderer>,
    started: bool,
    terminal: bool,
    console_clipped_rows: u8,
    // Live pixel/scratch allocations above are destroyed before their claims.
    output_claim: Option<MemoryClaim>,
    scratch_claim: Option<MemoryClaim>,
    // The original history pin releases last, after actual cursor/native/validation and claim disposal.
    request: SplashRenderRequest<T>,
}
impl<T: TranscriptView> SplashRenderTask<T> {
    pub(super) fn new(
        renderer: SplashRenderer,
        request: SplashRenderRequest<T>,
        lease: MemoryLease,
    ) -> Self {
        Self {
            renderer: Some(renderer),
            request,
            lease: Some(lease),
            raster: None,
            validator: None,
            started: false,
            terminal: false,
            console_clipped_rows: 0,
            output_claim: None,
            scratch_claim: None,
        }
    }
    /// Advance exactly one bounded owner stage or one <=32768-pixel validation slice.
    fn advance(&mut self) -> Result<Option<PreparedFrame>, RenderError> {
        if !self.started {
            self.started = true;
            if self.request.shape.scale() < 0.5 || !self.request.feedback.is_valid() {
                return Err(RenderError::InvalidFrame);
            }
            if self.renderer.as_ref().is_none_or(|renderer| renderer.cache_claim.is_none()) {
                return Err(RenderError::UnadmittedCache);
            }
            let lease = self.lease.as_ref().ok_or(RenderError::InvalidFrame)?;
            self.output_claim = Some(
                lease.claim(self.request.shape.byte_len() as u64).map_err(RenderError::Memory)?,
            );
            self.scratch_claim =
                Some(lease.claim(self.request.scratch_bytes()).map_err(RenderError::Memory)?);
            let raster = RenderStages::new(self.request.frame())?;
            if raster.capacity() != self.request.shape.byte_len() {
                return Err(RenderError::Allocation);
            }
            self.raster = Some(raster);
            return Ok(None);
        }
        if let Some(raster) = &mut self.raster {
            let lease = self.lease.as_ref().ok_or(RenderError::InvalidFrame)?;
            if raster.status_domain() {
                self.scratch_claim = None;
            } else if self.scratch_claim.is_none() {
                self.scratch_claim =
                    Some(lease.claim(self.request.scratch_bytes()).map_err(RenderError::Memory)?);
            }
            let renderer = self.renderer.as_mut().ok_or(RenderError::InvalidFrame)?;
            if !raster.step_admitted(
                renderer,
                Some(lease.clone()),
                self.request
                    .console
                    .as_ref()
                    .filter(|_| {
                        self.request.console_presentation
                            == sugarsubstitute_contracts::presentation::ConsolePresentation::Raster
                    })
                    .map(TranscriptView::transcript),
            )? {
                return Ok(None);
            }
            let raster = self.raster.take().ok_or(RenderError::InvalidFrame)?;
            self.console_clipped_rows = raster.console_clipped_rows();
            let pixels = raster.finish()?.take();
            self.scratch_claim = None;
            let lease = self.lease.take().ok_or(RenderError::InvalidFrame)?;
            let validator =
                FrameValidator::new(self.request.shape, self.request.identity, pixels, lease)
                    .map_err(RenderError::Frame)?;
            self.output_claim = None;
            self.validator = Some(validator);
            return Ok(None);
        }
        let validator = self.validator.as_mut().ok_or(RenderError::InvalidFrame)?;
        let max = NonZeroUsize::new(32768).ok_or(RenderError::InvalidFrame)?;
        if !validator.validate_next(max).map_err(RenderError::Frame)? {
            return Ok(None);
        }
        let validator = self.validator.take().ok_or(RenderError::InvalidFrame)?;
        Ok(Some(validator.finish().map_err(RenderError::Frame)?))
    }
    /// Return the renderer after disposing scratch and retaining only validated output ownership.
    fn complete(
        &mut self,
        frame: Result<PreparedFrame, RenderError>,
    ) -> Result<CpuStep<SplashPreparationResult<T>>, PreparationError> {
        if let Some(raster) = &self.raster {
            self.console_clipped_rows = raster.console_clipped_rows();
        }
        self.raster = None;
        self.validator = None;
        self.output_claim = None;
        self.scratch_claim = None;
        self.lease = None;
        self.terminal = true;
        let renderer = self.renderer.take().ok_or(PreparationError::InvalidTaskState)?;
        Ok(CpuStep::Complete(SplashPreparationResult {
            renderer,
            frame,
            console: self.request.console.take(),
            console_clipped_rows: self.console_clipped_rows,
        }))
    }
}
impl<T: TranscriptView> CpuTask<SplashPreparationResult<T>, PreparationError>
    for SplashRenderTask<T>
{
    /// Yield between bounded raster/PNG/validation slices; ordinary failures preserve caches.
    fn step(
        &mut self,
        _context: &JobContext,
    ) -> Result<CpuStep<SplashPreparationResult<T>>, PreparationError> {
        if self.terminal {
            return Err(PreparationError::InvalidTaskState);
        }
        match self.advance() {
            Ok(Some(frame)) => self.complete(Ok(frame)),
            Ok(None) => Ok(CpuStep::Yield),
            Err(error) => self.complete(Err(error)),
        }
    }
}

#[cfg(test)]
#[path = "render_preparation_tests.rs"]
mod tests;
