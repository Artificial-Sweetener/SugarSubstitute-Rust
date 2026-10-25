// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own native canvas windows and input while application services own image/document work.
use crate::DesktopError;
use std::{error::Error, path::PathBuf, sync::Arc, time::Duration};
use sugarsubstitute_canvas::{DamagePlan, FrameStamp};
use sugarsubstitute_contracts::execution::CompletionWake;
use sugarsubstitute_image_ops::Extent;

#[cfg(windows)]
#[allow(unsafe_code)]
mod paint;
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;
#[cfg(windows)]
pub use windows::run;

/// Borrowed opaque top-row-first sRGB words, `0xffRRGGBB`, with exact repair dependencies.
/// The application keeps the original execution-backed pixels alive for the complete borrow.
pub struct CpuFrame<'a> {
    /// Retained frame represented by every pixel in this buffer.
    pub target: FrameStamp,
    /// Native repair may blit only from this exact previously displayed frame.
    pub base: Option<FrameStamp>,
    /// Exact physical camera repair; exposure always uses the complete retained buffer.
    pub damage: &'a DamagePlan,
    /// Complete immutable retained viewport, including valid pixels outside dirty rectangles.
    pub pixels: &'a [u32],
}
impl CpuFrame<'_> {
    /// Validate complete source dimensions and dependency shape before any native pixel effect.
    /// # Errors
    /// Rejects incomplete pixel storage, unsupported dimensions or inconsistent frame identities.
    pub fn validate(&self) -> Result<Extent, DesktopError> {
        let extent = self.damage.dirty().surface();
        if self.pixels.len() as u64 != extent.pixels()
            || extent.width() > 16384
            || extent.height() > 16384
            || self.base.is_some_and(|base| {
                base.surface != self.target.surface || base.generation >= self.target.generation
            })
        {
            return Err(DesktopError::message(
                "canvas_frame",
                "Invalid native canvas frame extent or dependency",
            ));
        }
        Ok(extent)
    }
}

/// Initial native window presentation, with product copy supplied by localization ownership.
pub struct WindowOptions {
    /// Localized native window title.
    pub title: String,
    /// Initial physical client dimensions.
    pub extent: Extent,
    /// Show the window; native qualification may keep it off the input desktop.
    pub visible: bool,
}
/// Physical native observations, independent of image editing and backend selection policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    /// Pointer location in physical client pixels.
    Pointer([f64; 2]),
    /// Begin/end direct mouse panning; application owns its navigation state.
    Pan(bool),
    /// Native scrolling with its original device unit retained.
    Wheel(WheelDelta),
    /// Fit authored bounds into the current viewport.
    Fit,
    /// Display one document pixel per physical client pixel.
    ActualSize,
    /// Request a native image picker.
    Open,
    /// Cancel active drag state when native focus is lost.
    Unfocused,
}
/// Native wheel observations before application-owned zoom sensitivity is applied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WheelDelta {
    /// Horizontal and vertical line steps.
    Lines([f64; 2]),
    /// Horizontal and vertical physical device-pixel deltas.
    Pixels([f64; 2]),
}
/// Application lifetime composed into a native canvas event loop.
/// Every callback runs on the UI thread and must remain nonblocking; preparation uses execution.
pub trait Lifetime {
    /// Attach execution completion notification before submitting any work.
    /// # Errors
    /// Preserves partial startup ownership; close/poll/finish remain available after failure.
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>>;
    /// Update physical dimensions without waiting for old image preparation.
    /// # Errors
    /// Reports invalid geometry or admission failures.
    fn resize(&mut self, extent: Extent) -> Result<(), Box<dyn Error>>;
    /// Apply navigation/open intent and coalesce requested view changes.
    /// # Errors
    /// Preserves original camera or application lifecycle failures.
    fn input(&mut self, input: Input) -> Result<(), Box<dyn Error>>;
    /// Request one modal native picker; returning true consumes the pending request.
    fn take_picker_request(&mut self) -> bool;
    /// Receive the actual selected path or ordinary cancellation.
    /// # Errors
    /// Reports native shell or application image-loading failures.
    fn selected(
        &mut self,
        selection: Result<Option<PathBuf>, Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>>;
    /// Observe completions and advance application-owned rendering/teardown without waiting.
    /// # Errors
    /// Preserves original asynchronous and native lifecycle failures.
    fn poll(&mut self, elapsed: Duration) -> Result<(), Box<dyn Error>>;
    /// Borrow the latest complete retained CPU viewport.
    fn frame(&self) -> Option<CpuFrame<'_>>;
    /// Current localized title/status, independent of paint scheduling.
    fn title(&self) -> &str;
    /// Earliest application refinement deadline in the native clock epoch; no timer is spawned.
    fn next_deadline(&self) -> Option<std::time::Instant>;
    /// Observe application-requested closure without manufacturing an OS input event.
    /// Returning true starts the same cancellation and native retirement path as window close.
    fn is_closing(&self) -> bool;
    /// Request cancellation; actual terminal observation still belongs to poll/settled.
    /// # Errors
    /// Preserves original scope/worker shutdown failures.
    fn close(&mut self) -> Result<(), Box<dyn Error>>;
    /// Observe actual worker and resource settlement after close was requested.
    /// # Errors
    /// Reports genuine shutdown failure without treating cancellation as termination.
    fn settled(&mut self) -> Result<bool, Box<dyn Error>>;
    /// Finish already-stopped execution after native window destruction.
    /// # Errors
    /// Preserves worker joins, resource retirement and original application failures.
    fn finish(&mut self) -> Result<(), Box<dyn Error>>;
}
