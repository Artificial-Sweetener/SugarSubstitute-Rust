// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Define presentation-only values that do not own application startup policy.

use std::{error::Error, fmt, time::Duration};

/// Configure the native surface in logical pixels.
pub struct DesktopOptions {
    /// Localized native window identity.
    pub title: String,
    /// Initial client width in logical pixels.
    pub logical_width: u32,
    /// Initial client height in logical pixels.
    pub logical_height: u32,
    /// Select dark native material and portable fallback color.
    pub dark: bool,
    /// Automatically close qualification runs after this duration.
    pub duration: Option<Duration>,
    /// Retain an in-memory whole-frame audit for explicit native qualification.
    /// Callers normally disable this; recording allocates no background work or files.
    pub audit_frames: bool,
}

/// Report measured presentation work without making launch-speed assumptions.
#[derive(Debug)]
pub struct DesktopReceipt {
    /// Time from entering the native adapter to the first completed frame submission.
    pub first_frame: Duration,
    /// Successfully submitted frames during this native window lifetime.
    pub frames_presented: u64,
    /// Native material selected by the platform adapter, independent of content pixels.
    pub material: NativeMaterial,
    /// Combined CPU render, validation and native submission work for successful frames.
    pub total_frame_work: Duration,
    /// Combined intervals between consecutive successful frame submissions.
    pub total_frame_intervals: Duration,
    /// Longest measured submission interval; zero when fewer than two frames were presented.
    /// Covers the complete surface lifetime, including intervals outside movement
    /// and gaps at move-loop entry/exit; move samples identify in-loop submissions separately.
    pub longest_frame_interval: Duration,
    /// Optional full-source audit collected after successful native presentations.
    /// Pixel ownership transfers to the caller after native window teardown.
    pub frame_audit: Option<FrameAuditReceipt>,
}

/// Report every whole-frame source transition in an explicitly audited native run.
///
/// Comparison includes device dimensions and every premultiplied RGBA byte.
/// Native focus overlays and compositor effects are outside this source audit.
#[derive(Debug)]
pub struct FrameAuditReceipt {
    /// First successfully presented source frame, retained with its original allocation.
    pub first_frame: Frame,
    /// Total valid frames whose native presentation and control painting succeeded.
    pub presented_frames: u64,
    /// Transitions from the preceding presented source, including change then restoration.
    pub content_changes: u64,
    /// At most 512 successful source submissions bracketed by owned native move-loop observations.
    /// These observations establish submission within the loop, not compositor frame age
    /// or coverage of the gaps at move-loop entry and exit.
    pub move_presentations: Vec<MovePresentation>,
    /// All successful submissions bracketed by owned native move-loop observations.
    pub move_frames_observed: u64,
    /// Qualifying submissions omitted after the 512-sample bound; nonzero means incomplete evidence.
    pub move_samples_omitted: u64,
}

/// Identify one successful source submission during an observed owned native move loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovePresentation {
    /// One-based successful submission sequence over the complete native surface lifetime.
    pub sequence: u64,
    /// Time from entering the native adapter to the post-submission move-state observation.
    pub elapsed: Duration,
    /// Complete-source transitions since the first submission, including transitions outside movement.
    pub source_revision: u64,
}

/// Describe the actual native background selected by the platform adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeMaterial {
    /// Windows DWM accepted the main-window Mica backdrop.
    Mica,
    /// CPU pixels are composited over an explicit native theme color.
    ThemedFallback,
}

/// Carry an owned CPU-rasterized image in premultiplied RGBA byte order.
#[derive(Debug)]
pub struct Frame {
    /// Width in device pixels.
    pub width: u32,
    /// Height in device pixels.
    pub height: u32,
    /// Exactly width times height times four bytes, top row first.
    pub premultiplied_rgba: Vec<u8>,
}

impl Frame {
    /// Reject malformed external buffers before native presentation reads them.
    pub fn validate(&self) -> Result<(), DesktopError> {
        let required = usize::try_from(self.width)
            .ok()
            .and_then(|width| {
                usize::try_from(self.height).ok().and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(4));
        if self.width == 0 || self.height == 0 || required != Some(self.premultiplied_rgba.len()) {
            return Err(DesktopError::message(
                "validate_frame",
                "invalid frame dimensions or byte length",
            ));
        }
        if self
            .premultiplied_rgba
            .chunks_exact(4)
            .any(|pixel| pixel[..3].iter().any(|channel| *channel > pixel[3]))
        {
            return Err(DesktopError::message(
                "validate_frame",
                "frame channels exceed premultiplied alpha",
            ));
        }
        Ok(())
    }
}

/// Express pointer coordinates in logical client pixels.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pointer {
    /// Horizontal position.
    pub x: f64,
    /// Vertical position.
    pub y: f64,
}

/// Identify a presentation interaction without exposing native window handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitTarget {
    /// Drag the native window using the operating system's move loop.
    Caption,
    /// Close this native surface.
    Close,
    /// Let the content own its client interaction without native window actions.
    Client,
}

/// Forward pointer state to the presentation owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerAction {
    /// Pointer position changed.
    Move,
    /// Pointer left the client surface.
    Leave,
    /// Primary button was pressed.
    Press,
    /// Primary button was released.
    Release,
}

/// Forward an explicit native keyboard intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// Forward Tab without assigning application-specific behavior.
    Tab,
}

/// Identify one native accessible pushbutton owned by the presentation content.
///
/// IDs are nonzero, unique within the surface, and stable for its lifetime. Labels
/// are localized by the content owner; bounds are logical client pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AccessibleControl {
    /// Stable surface-local native control identifier.
    pub id: u16,
    /// Localized current action exposed to assistive technology.
    pub label: &'static str,
    /// Logical position and size of the painted control.
    pub bounds: ControlBounds,
    /// Route activation to the surface's native close operation.
    pub close: bool,
}

/// Bound an accessible control to its painted logical client rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlBounds {
    /// Left coordinate in logical client pixels.
    pub x: f64,
    /// Top coordinate in logical client pixels.
    pub y: f64,
    /// Positive width in logical pixels.
    pub width: f64,
    /// Positive height in logical pixels.
    pub height: f64,
}

/// Supply frames and accept UI input on the native event-loop thread.
pub trait WindowContent {
    /// Rasterize the requested device size; elapsed time starts at native adapter entry.
    fn render(
        &mut self,
        width: u32,
        height: u32,
        scale: f64,
        elapsed: Duration,
    ) -> Result<Frame, Box<dyn Error>>;
    /// Classify logical pointer positions for native close and drag behavior.
    fn hit_test(&self, pointer: Pointer) -> HitTarget;
    /// Receive pointer activity; return true only when visible state changed.
    fn pointer(&mut self, action: PointerAction, pointer: Pointer) -> bool;
    /// Receive keyboard intent; return true only when visible state changed.
    fn key(&mut self, action: KeyAction) -> bool;
    /// Report whether the current presentation needs a continuous animation cadence.
    /// Static content waits for native expose, input, or application events.
    fn animating(&self) -> bool;
    /// Borrow the complete accessible pushbutton snapshot for this surface.
    /// Identities and geometry match the painted controls; the adapter owns native peers.
    fn accessible_controls(&self) -> &[AccessibleControl];
    /// Activate a content-owned control by its registered identity; return visible dirty state.
    /// Native-close controls are consumed by the adapter and never forwarded here.
    fn activate(&mut self, id: u16) -> bool;
}

/// Preserve the failed native operation and its underlying cause.
#[derive(Debug)]
pub struct DesktopError {
    operation: &'static str,
    cause: Box<dyn Error>,
}

impl DesktopError {
    /// Attach an external failure to its adapter operation.
    pub(crate) fn new(operation: &'static str, cause: impl Error + 'static) -> Self {
        Self { operation, cause: Box::new(cause) }
    }
    /// Attach already boxed content errors without losing their cause.
    pub(crate) fn boxed(operation: &'static str, cause: Box<dyn Error>) -> Self {
        Self { operation, cause }
    }
    /// Describe a violated native contract or platform error code.
    pub(crate) fn message(operation: &'static str, message: impl Into<String>) -> Self {
        Self::new(operation, std::io::Error::other(message.into()))
    }
}

impl fmt::Display for DesktopError {
    /// Include operation identity in an actionable diagnostic.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.operation, self.cause)
    }
}

impl Error for DesktopError {
    /// Preserve the original external error for diagnostics.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::Frame;

    /// Accept a transparent raster while rejecting mismatched dimensions and straight alpha.
    #[test]
    fn frame_contract_bounds_native_reads() {
        let mut frame = Frame { width: 1, height: 1, premultiplied_rgba: vec![0, 0, 0, 0] };
        assert!(frame.validate().is_ok());
        frame.width = 2;
        assert!(frame.validate().is_err());
        frame.width = 1;
        frame.premultiplied_rgba = vec![255, 0, 0, 128];
        assert!(frame.validate().is_err());
    }
}
