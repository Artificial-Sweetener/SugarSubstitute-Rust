// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Define presentation-only values that do not own application startup policy.

use std::{error::Error, fmt, num::NonZeroUsize, sync::Arc, time::Duration};
use sugarsubstitute_contracts::{
    execution::CompletionWake,
    presentation::{FrameShape, PreparedFrame},
};

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
    /// Record bounded raw Windows callback/submission evidence for explicit qualification.
    /// Other platforms reject this opt-in; disabled runs allocate no timing buffers.
    pub audit_timings: bool,
}

/// Report measured presentation work without making launch-speed assumptions.
#[derive(Debug)]
pub struct DesktopReceipt {
    /// Time from entering the native adapter to the first completed frame submission.
    pub first_frame: Duration,
    /// Successfully submitted frames during this native window lifetime.
    pub frames_presented: u64,
    /// Successful submissions whose identity differs from the most recently submitted frame.
    pub prepared_frames_presented: u64,
    /// Successful native expose/focus submissions reusing the most recently submitted identity.
    pub retained_frame_exposes: u64,
    /// Native material selected by the platform adapter, independent of content pixels.
    pub material: NativeMaterial,
    /// Combined native submission and optional source audit work for successful frames.
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
    /// Optional bounded raw native callback/submission observations after full teardown.
    pub timings: Option<crate::timing::NativeTimingReceipt>,
}

/// Report every whole-frame source transition in an explicitly audited native run.
///
/// Comparison includes device dimensions and every premultiplied RGBA byte.
/// Native focus overlays and compositor effects are outside this source audit.
#[derive(Debug)]
pub struct FrameAuditReceipt {
    /// First successfully presented source frame, retained with its original allocation.
    pub first_frame: PreparedFrame,
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
    /// Expose this stable peer and enable its callback action; false hides and disables it.
    pub visible: bool,
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

/// Supply owned prepared frames and accept UI input on the native event-loop thread.
///
/// Every fallible method preserves the underlying owner/adapter error in its boxed
/// cause. Calls return without waiting for preparation or unresolved shutdown work.
/// A failed start still leaves this object responsible for request/poll/finish shutdown.
pub trait WindowContent {
    /// Start owned preparation after a thread-safe native wake is available.
    ///
    /// Called once on the UI thread. It never waits for preparation; early wakes
    /// may be queued before the event loop begins processing. Failures preserve
    /// causes and still require the owned shutdown protocol.
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>>;
    /// Submit/coalesce preparation for validated device geometry in constant time.
    ///
    /// Apply changed shape/state invalidation before admission or completion publication.
    /// Elapsed time starts at native adapter entry. The owner handles expected
    /// busy/admission decisions internally; this call never rasterizes or waits.
    fn request_frame(&mut self, shape: FrameShape, elapsed: Duration)
    -> Result<(), Box<dyn Error>>;
    /// Drain at most `min(max, 32)` terminal metadata and acknowledge typed results.
    ///
    /// Return the newest publishable immutable frame, retaining its memory claim.
    /// Re-wake remaining metadata. Closing/hidden polling still disposes terminal
    /// results; acknowledgement does not release retained frame allocations.
    fn poll_frame(&mut self, max: NonZeroUsize) -> Result<Option<PreparedFrame>, Box<dyn Error>>;
    /// Invalidate visibility state before cancellation or subsequent preparation.
    ///
    /// Hidden/minimized surfaces admit no preparation; already accepted work must
    /// still reach observed terminal disposal. Called on the UI thread.
    fn set_visible(&mut self, visible: bool) -> Result<(), Box<dyn Error>>;
    /// Close admission and request owned shutdown without waiting; idempotent.
    fn request_shutdown(&mut self) -> Result<(), Box<dyn Error>>;
    /// Poll actual job, terminal, notification and worker settlement without waiting.
    ///
    /// `true` proves settlement; cancellation requests alone do not. The native
    /// event loop remains alive until this succeeds, including after errors.
    fn poll_shutdown(&mut self) -> Result<bool, Box<dyn Error>>;
    /// Finish settled ownership after native controls, presenter, window and loop drop.
    ///
    /// This is called only after `poll_shutdown` returned true. Preserve underlying
    /// teardown errors; unresolved owned work must never be joined on the UI thread.
    /// Success confirms final joins and disposal. An error leaves final custody uncertain;
    /// the native adapter retains the consumed content owner and any acquired STA reference.
    fn finish_shutdown(&mut self) -> Result<(), Box<dyn Error>>;
    /// Classify logical pointer positions for native close and drag behavior.
    fn hit_test(&self, pointer: Pointer) -> HitTarget;
    /// Receive pointer activity; return true only when visible state changed.
    fn pointer(&mut self, action: PointerAction, pointer: Pointer) -> bool;
    /// Receive keyboard intent; return true only when visible state changed.
    fn key(&mut self, action: KeyAction) -> bool;
    /// Report whether the current presentation needs a continuous animation cadence.
    /// Static content waits for native expose, input, or application events.
    fn animating(&self) -> bool;
    /// Borrow every stable accessible peer for this surface, including hidden controls.
    /// IDs and native-close actions stay fixed; visibility changes preserve the peer lifetime.
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

#[path = "desktop_failure_diagnostic.rs"]
mod failure_diagnostic;

impl DesktopError {
    /// Borrow only the stable operation for feature-only closed native diagnostics.
    #[cfg(all(feature = "k1-native-qualification", any(windows, test)))]
    pub(crate) fn qualification_operation(&self) -> &'static str {
        self.operation
    }

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

    /// Keep the original returned cause and report subsequent failures without retaining a list.
    pub(crate) fn retain_first(failure: &mut Option<Self>, error: Self) {
        let first = failure.is_none();
        Self::retain_first_with(failure, error, |error| {
            tracing::error!(
                operation = error.operation,
                cause = %error.cause,
                "Additional failure during owned native shutdown"
            );
        });
        // Adopt the original cause before a logging subscriber can observe it.
        if first && let Some(original) = failure {
            failure_diagnostic::emit(original.operation, original.cause.as_ref());
        }
    }

    /// Separate bounded primary storage from the structured secondary diagnostic sink.
    fn retain_first_with(failure: &mut Option<Self>, error: Self, report: impl FnOnce(&Self)) {
        if failure.is_some() {
            report(&error);
        } else {
            *failure = Some(error);
        }
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
    use super::*;

    /// Preserve the primary typed source while every later cleanup cause reaches the sink.
    #[test]
    fn secondary_shutdown_errors_are_observed_without_replacing_or_accumulating_primary()
    -> Result<(), Box<dyn Error>> {
        let mut failure = None;
        let mut primary_reports = 0;
        DesktopError::retain_first_with(
            &mut failure,
            DesktopError::new("prepare", std::io::Error::other("original cause")),
            |_| primary_reports += 1,
        );
        assert_eq!(primary_reports, 0);
        let mut reports = [None, None];
        for (index, operation) in ["shutdown_controls", "finish_shutdown"].into_iter().enumerate() {
            DesktopError::retain_first_with(
                &mut failure,
                DesktopError::new(operation, std::io::Error::other("cleanup cause")),
                |error| {
                    reports[index] = Some((error.operation, error.cause.to_string()));
                },
            );
        }
        assert_eq!(
            reports,
            [
                Some(("shutdown_controls", "cleanup cause".to_owned())),
                Some(("finish_shutdown", "cleanup cause".to_owned())),
            ]
        );
        let original = failure.ok_or("Primary cause lost")?;
        assert_eq!(original.to_string(), "prepare: original cause");
        assert_eq!(
            original
                .source()
                .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                .ok_or("Typed original cause lost")?
                .to_string(),
            "original cause"
        );
        Ok(())
    }
}
