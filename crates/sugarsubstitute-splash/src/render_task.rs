// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Move an exclusive renderer through admitted initialization and bounded frame preparation.

use super::{RenderError, RenderFrame, SplashRenderer, stages::RenderStages};
use crate::{AnimationFrame, Appearance, ProgressFrame};
use std::{error::Error, fmt, num::NonZeroUsize};
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

/// Capture one immutable view revision for a retained exclusive renderer.
#[derive(Clone, Copy)]
pub struct SplashRenderRequest {
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
}
impl SplashRenderRequest {
    /// Bound output plus live scratch and the host's later RGBA/BGRA retained pair.
    ///
    /// At most two one-byte masks coexist, with fixed geometry overhead. PNG scratch
    /// and bounded native glyph buffers reuse the six-MiB stage allowance. During
    /// validator handoff the same output is conservatively charged twice until its
    /// previous claim is dropped; no pixel copy occurs. Allocation capacity is checked.
    pub fn reservation_bytes(&self) -> u64 {
        let output = self.shape.byte_len() as u64;
        (2 * output).max(output + self.scratch_bytes())
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
pub struct SplashPreparationResult {
    /// Exclusive owner recovered after actual job completion, including ordinary failures.
    pub renderer: SplashRenderer,
    /// Immutable prepared pixels or the original typed rendering error.
    pub frame: Result<PreparedFrame, RenderError>,
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
pub struct SplashRenderTask {
    request: SplashRenderRequest,
    lease: Option<MemoryLease>,
    raster: Option<RenderStages>,
    validator: Option<FrameValidator>,
    // Pending decoded cache pixels are destroyed before the renderer's cache claim.
    renderer: Option<SplashRenderer>,
    started: bool,
    terminal: bool,
    // Live pixel/scratch allocations above are destroyed before their claims.
    output_claim: Option<MemoryClaim>,
    scratch_claim: Option<MemoryClaim>,
}
impl SplashRenderTask {
    pub(super) fn new(
        renderer: SplashRenderer,
        request: SplashRenderRequest,
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
            if !raster.step_admitted(renderer, Some(lease.clone()))? {
                return Ok(None);
            }
            let raster = self.raster.take().ok_or(RenderError::InvalidFrame)?;
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
    ) -> Result<CpuStep<SplashPreparationResult>, PreparationError> {
        self.raster = None;
        self.validator = None;
        self.output_claim = None;
        self.scratch_claim = None;
        self.lease = None;
        self.terminal = true;
        let renderer = self.renderer.take().ok_or(PreparationError::InvalidTaskState)?;
        Ok(CpuStep::Complete(SplashPreparationResult { renderer, frame }))
    }
}
impl CpuTask<SplashPreparationResult, PreparationError> for SplashRenderTask {
    /// Yield between bounded raster/PNG/validation slices; ordinary failures preserve caches.
    fn step(
        &mut self,
        _context: &JobContext,
    ) -> Result<CpuStep<SplashPreparationResult>, PreparationError> {
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
