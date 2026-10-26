// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own native canvas windows and input while application services own image/document work.
use crate::DesktopError;
use std::{error::Error, path::PathBuf, sync::Arc, time::Duration};
use sugarsubstitute_canvas::{CursorIntent, DamagePlan, FrameStamp, ToolInput};
use sugarsubstitute_contracts::execution::CompletionWake;
use sugarsubstitute_image_gpu::{GpuDevice, GpuError, WindowFrame, WindowSurface};
use sugarsubstitute_image_ops::Extent;

/// Desktop-owned peer custody, attached to GPU presentation only on the original window thread.
#[derive(Clone)]
pub struct WindowTarget(Arc<tao::window::Window>);
impl WindowTarget {
    /// Bind an already-selected native device to this peer while desktop ownership stays retained.
    /// # Errors
    /// Preserves native graphics/admission errors and rejects incompatible window/device pairs.
    pub fn attach_gpu(&self, device: Arc<GpuDevice>) -> Result<WindowSurface, GpuError> {
        WindowSurface::attach(device, self.0.clone())
    }
}
/// Completed native GPU presentation with the exact application frame identity it represents.
pub struct GpuPresentation {
    /// Actual completed application frame.
    pub target: FrameStamp,
    /// Already-drawn native swapchain image; presentation performs no worker wait.
    pub frame: WindowFrame,
}

#[cfg(any(windows, test))]
mod feedback;
#[cfg(windows)]
#[allow(unsafe_code)]
mod paint;
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;
#[cfg(windows)]
pub use windows::run;

/// Native meaning of a borrowed CPU viewport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpuFrameKind {
    /// Canonical retained pixels that may use an exact preceding-frame repair dependency.
    Prepared,
    /// Complete display projection; exact same-source integer motion may reuse native pixels.
    Projected,
}

/// Borrowed opaque top-row-first sRGB words, `0xffRRGGBB`, with explicit pixel semantics.
/// The application keeps the original execution-backed pixels alive for the complete borrow.
pub struct CpuFrame<'a> {
    /// Whether these pixels are canonical prepared output or a transient complete projection.
    pub kind: CpuFrameKind,
    /// `true` requires exactly one successful-upload acknowledgement from whichever paint route
    /// presents this frame.
    pub pending: bool,
    /// Retained frame represented by every pixel in this buffer.
    pub target: FrameStamp,
    /// Native repair may blit only from this exact previously displayed frame. For projections,
    /// the producer must prove identical source identity and unchanged output sampling phase.
    pub base: Option<FrameStamp>,
    /// Physical output repair, independent of any offscreen backing repair.
    /// Exposure always uses the complete retained buffer.
    pub damage: &'a DamagePlan,
    /// Complete immutable retained viewport, including valid pixels outside dirty rectangles.
    pub pixels: &'a [u32],
}
impl CpuFrame<'_> {
    /// Validate complete source dimensions and dependency shape before any native pixel effect.
    /// Projections carry complete pixels for exposure; partial output repair requires an exact base.
    /// # Errors
    /// Rejects incomplete pixel storage, unsupported dimensions or inconsistent frame identities.
    pub fn validate(&self) -> Result<Extent, DesktopError> {
        let extent = self.damage.dirty().surface();
        let invalid_projection = self.kind == CpuFrameKind::Projected
            && self.base.is_none()
            && (self.damage.blit().is_some()
                || self.damage.dirty().regions().ne([extent.region()]));
        if self.pixels.len() as u64 != extent.pixels()
            || extent.width() > 16384
            || extent.height() > 16384
            || self.base.is_some_and(|base| {
                base.surface != self.target.surface || base.generation >= self.target.generation
            })
            || invalid_projection
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
/// Physical observations assigned to this canvas by desktop/interface hit testing and focus.
/// Interface controls consume their observations before this boundary; transparent presentation
/// alone does not establish an input target. Image editing and backend policy remain independent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    /// Tool observation normalized once into physical pixels and hardware wheel steps.
    Tool(ToolInput),
    /// Non-repeating Space state, independent of persistent tool selection.
    TemporaryNavigation(bool),
    /// Current active display rate in hertz; unavailable timing selects the portable fallback.
    DisplayRefresh(Option<f64>),
    /// Request a native image picker.
    Open,
    /// Cancel active drag state when native focus is lost.
    Unfocused,
}
/// Application interaction intent; the desktop supplies actual native resources and capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Feedback {
    /// Cursor shape resolved by the effective canvas tool.
    pub cursor: CursorIntent,
    /// Whether the current gesture requires native pointer capture.
    pub capture: bool,
}
/// Application lifetime composed into a native canvas event loop.
/// Every callback runs on the UI thread and must remain nonblocking; preparation uses execution.
pub trait Lifetime {
    /// Attach execution completion notification before submitting any work.
    /// # Errors
    /// Preserves partial startup ownership; close/poll/finish remain available after failure.
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>>;
    /// Receive desktop peer custody on its owning thread before any image work is submitted.
    /// # Errors
    /// Reports duplicate attachment or partial native startup failures.
    fn attach_window(&mut self, target: WindowTarget) -> Result<(), Box<dyn Error>>;
    /// Update physical dimensions without waiting for old image preparation.
    /// # Errors
    /// Reports invalid geometry or admission failures.
    fn resize(&mut self, extent: Extent) -> Result<(), Box<dyn Error>>;
    /// Apply input after desktop/interface hit testing assigns it to this canvas surface.
    /// Overlaid controls keep their own pointer and keyboard focus. Captured canvas gestures
    /// continue until release or explicit cancellation; UI toolkit events do not cross this port.
    /// # Errors
    /// Preserves original camera or application lifecycle failures.
    fn input(&mut self, input: Input) -> Result<(), Box<dyn Error>>;
    /// Current effective-tool feedback without native handles or image preparation.
    fn feedback(&self) -> Feedback;
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
    /// Acknowledge successful synchronous upload of a new CPU frame to the native surface.
    /// This confirms native pixel acceptance on the UI thread, not physical display scanout.
    /// # Errors
    /// Rejects foreign, duplicate or out-of-order desktop acknowledgements.
    fn did_present_cpu(&mut self, target: FrameStamp) -> Result<(), Box<dyn Error>>;
    /// Take at most one completed native GPU presentation; ownership is consumed by presentation.
    fn take_gpu_presentation(&mut self) -> Option<GpuPresentation>;
    /// Acknowledge the exact completed frame after the native queue accepted presentation.
    /// # Errors
    /// Rejects foreign, duplicate or out-of-order desktop acknowledgements.
    fn did_present_gpu(&mut self, target: FrameStamp) -> Result<(), Box<dyn Error>>;
    /// Request restoration of native GPU pixels after exposure without doing image work on the UI.
    fn expose(&mut self);
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
