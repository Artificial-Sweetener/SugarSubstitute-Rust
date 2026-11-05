// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own native canvas windows and input while application services own image/document work.
use crate::DesktopError;
use std::{error::Error, path::PathBuf, sync::Arc, time::Duration};
use sugarsubstitute_canvas::{CursorIntent, DamagePlan, FrameStamp, ToolInput};
use sugarsubstitute_contracts::execution::CompletionWake;
use sugarsubstitute_image_gpu::{GpuDevice, GpuError, WindowPresentation, WindowSurface};
use sugarsubstitute_image_ops::Extent;

/// Convert validated web geometry into bounded native client coordinates.
pub mod placement;

/// Main-window canvas acknowledgement rules shared by the compositor and application tests.
#[cfg(feature = "native-composition")]
pub mod acknowledgement;

#[cfg(feature = "native-composition")]
pub(crate) mod drag_preview;
#[cfg(feature = "native-composition")]
pub use drag_preview::{DRAG_PREVIEW_CEILING, DragPreview};
#[cfg(feature = "native-composition")]
mod gesture;
#[cfg(feature = "native-composition")]
pub use gesture::{
    MAX_INPUT_OCCLUSIONS, MainCanvasButton, MainCanvasDisplay, MainCanvasDragOut,
    MainCanvasGesture, MainCanvasInput, MainCanvasInputRegion,
};
/// Composition-device adapter placement and shared-texture fallback reasons.
pub mod composition_adapter;

#[cfg(all(windows, feature = "native-composition"))]
#[allow(unsafe_code)]
mod graphics;
#[cfg(all(windows, feature = "native-composition"))]
pub use graphics::{CanvasGraphics, CanvasGraphicsError};
#[cfg(all(windows, feature = "native-composition"))]
#[allow(unsafe_code)]
mod composition_fence;
#[cfg(all(windows, feature = "native-composition"))]
pub use composition_fence::{CompositionFence, CompositionFenceError, CompositionWait};
#[cfg(all(windows, feature = "native-composition"))]
#[allow(unsafe_code)]
mod shared_texture;
#[cfg(all(windows, feature = "native-composition"))]
pub use shared_texture::{SharedCanvasTexture, SharedTextureFactory};

/// Maximum supported physical width or height for a native canvas viewport.
pub const MAX_VIEWPORT_EDGE: u32 = 16_384;

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
/// Native queue acceptance returned by a presentation worker, with its exact application identity.
/// Contains no GPU resource or operation that can block the window thread.
pub struct GpuPresentation {
    /// Actual application frame accepted by the native queue.
    pub target: FrameStamp,
    /// Immutable queue receipt; acceptance does not prove compositor delivery or scanout.
    pub receipt: WindowPresentation,
}

#[cfg(windows)]
pub(crate) mod click;
#[cfg(any(windows, test))]
mod feedback;
#[cfg(any(windows, test))]
mod latency;
#[cfg(windows)]
#[allow(unsafe_code)]
mod paint;
#[cfg(all(windows, test))]
pub(crate) mod test_harness;
#[cfg(windows)]
#[allow(unsafe_code)]
pub(crate) mod windows;
#[cfg(windows)]
pub use windows::run;

/// Physical `[width, height]` of the box enclosing every attached display, the largest canvas
/// any window on them can show.
/// # Errors
/// Reports a failed or empty native display topology query.
#[cfg(windows)]
pub fn desktop_extent() -> Result<[u32; 2], DesktopError> {
    windows::display::desktop_extent()
}

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
#[derive(Clone, Copy)]
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

/// One prepared canvas frame for the exact current main-window composition.
/// The viewport and clip must have been projected from the current native WebView allocation.
#[cfg(feature = "native-composition")]
pub struct MainCanvasFrame<'a> {
    /// Current native main identity, including its navigation generation.
    pub identity: crate::MainIdentity,
    /// Complete canvas viewport and conservative visible clip in physical client pixels.
    pub placement: placement::CanvasPlacement,
    /// Native WebView allocation used to project the current geometry.
    pub allocation: placement::WebViewAllocation,
    /// Already admitted storage shared with the application-owned canvas lifetime.
    pub memory: &'a sugarsubstitute_contracts::execution::MemoryLease,
    /// Worker-prepared graphics device retained by the canvas and native surface.
    #[cfg(windows)]
    pub graphics: &'a Arc<CanvasGraphics>,
    /// Complete immutable prepared pixels and their application frame identity.
    pub content: CanvasContent<'a>,
}

/// Complete pixels of one main-window canvas frame, prepared by either renderer.
/// The same retained surface accepts both kinds, so presentation may switch per frame.
#[cfg(feature = "native-composition")]
pub enum CanvasContent<'a> {
    /// CPU-prepared pixels uploaded through Direct2D.
    Cpu(CpuFrame<'a>),
    /// A completed GPU frame in a shared composition texture.
    #[cfg(windows)]
    Shared(SharedFrame<'a>),
}

/// A GPU-drawn frame whose shared texture the composition copies on the UI thread.
/// The drawing worker observed GPU completion before this frame was published.
#[cfg(all(windows, feature = "native-composition"))]
pub struct SharedFrame<'a> {
    /// `true` requires exactly one successful-commit acknowledgement for this frame.
    pub pending: bool,
    /// Retained frame represented by every texture pixel.
    pub target: FrameStamp,
    /// Shared texture exported by the frame's composition graphics device.
    pub texture: &'a SharedCanvasTexture,
}

#[cfg(feature = "native-composition")]
impl CanvasContent<'_> {
    /// Application frame represented by these pixels.
    pub fn target(&self) -> FrameStamp {
        match self {
            Self::Cpu(frame) => frame.target,
            #[cfg(windows)]
            Self::Shared(frame) => frame.target,
        }
    }
    /// Borrow CPU-prepared pixels; shared GPU frames expose no host pixels.
    pub fn cpu_pixels(&self) -> &[u32] {
        match self {
            Self::Cpu(frame) => frame.pixels,
            #[cfg(windows)]
            Self::Shared(_) => &[],
        }
    }
    /// Whether presentation must acknowledge this frame's first successful commit.
    pub fn pending(&self) -> bool {
        match self {
            Self::Cpu(frame) => frame.pending,
            #[cfg(windows)]
            Self::Shared(frame) => frame.pending,
        }
    }
    /// Validate complete pixel dimensions before any native effect.
    /// # Errors
    /// Rejects invalid CPU frames and shared textures above the viewport bound.
    pub fn validate(&self) -> Result<Extent, DesktopError> {
        match self {
            Self::Cpu(frame) => frame.validate(),
            #[cfg(windows)]
            Self::Shared(frame) => {
                let extent = frame.texture.extent();
                if extent.width() > MAX_VIEWPORT_EDGE || extent.height() > MAX_VIEWPORT_EDGE {
                    return Err(DesktopError::message(
                        "canvas_frame",
                        "Shared canvas texture exceeds the viewport bound",
                    ));
                }
                Ok(extent)
            }
        }
    }
}

/// Keep only the exact native pixels already acknowledged, shown at the current geometry.
/// This carries no image storage and cannot upload or acknowledge a replacement.
#[cfg(feature = "native-composition")]
#[derive(Clone, Copy)]
pub struct MainCanvasRetention {
    /// Authorized main window and navigation generation.
    pub identity: crate::MainIdentity,
    /// Previously acknowledged frame; never the incoming source's identity.
    pub target: FrameStamp,
    /// Previously acknowledged physical placement, unchanged during retention.
    pub placement: placement::CanvasPlacement,
    /// Where the retained pixels show now: the acknowledged placement, or the surface's current
    /// geometry while a frame at that geometry is prepared. Changed geometry keeps the retained
    /// pixels centred in the current viewport and clipped to its clip, as a resized camera keeps
    /// its centre.
    pub shown: placement::CanvasPlacement,
    /// Current native allocation that qualifies `shown`.
    pub allocation: placement::WebViewAllocation,
}

/// Application intent for the native surface on this event-loop turn.
#[cfg(feature = "native-composition")]
pub enum MainCanvasPresentation<'a> {
    /// Withdraw native pixels, including on lost attachment or backend authority.
    Hide,
    /// Keep a qualified native surface while its replacement is being prepared.
    Retain(MainCanvasRetention),
    /// Upload the current prepared frame and observe its exact acknowledgement.
    Frame(MainCanvasFrame<'a>),
}

#[cfg(feature = "native-composition")]
impl MainCanvasPresentation<'_> {
    /// Identify the native lifetime whose backend grant must still be current.
    pub fn identity(&self) -> Option<crate::MainIdentity> {
        match self {
            Self::Hide => None,
            Self::Retain(retained) => Some(retained.identity),
            Self::Frame(frame) => Some(frame.identity),
        }
    }
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
            || extent.width() > MAX_VIEWPORT_EDGE
            || extent.height() > MAX_VIEWPORT_EDGE
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
    /// Whether the native viewport can currently display pixels. Minimized or hidden tabs
    /// retain image products and demand while native presentation stays suspended.
    Drawable(bool),
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
    /// Take at most one worker-completed native queue receipt without performing GPU operations.
    fn take_gpu_presentation(&mut self) -> Option<GpuPresentation>;
    /// Acknowledge the exact completed frame after the native queue accepted presentation.
    /// # Errors
    /// Rejects foreign, duplicate or out-of-order desktop acknowledgements.
    fn did_present_gpu(&mut self, target: FrameStamp) -> Result<(), Box<dyn Error>>;
    /// Request retained native GPU presentation after exposure or pointer activity.
    /// This coalesces display demand without changing authored content or evaluating image ops.
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
