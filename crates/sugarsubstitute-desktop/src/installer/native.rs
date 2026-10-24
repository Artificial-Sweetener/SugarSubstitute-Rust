// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native installer window contract. Application navigation and pixel preparation remain outside the adapter.
use std::{error::Error, sync::Arc, time::Duration};
use sugarsubstitute_contracts::{
    execution::CompletionWake,
    presentation::{FrameShape, PreparedFrame},
};
use sugarsubstitute_ui::{
    form::{Control, ControlId, Input},
    widgets::Bounds,
};

/// Initial native window identity; qualification keeps the same original window covered.
pub struct Window {
    /// Localized operating-system window title.
    pub title: String,
    /// Show the original window after its first complete native presentation.
    pub visible: bool,
    /// Stable close button in the application's retained native inventory.
    pub close: ControlId,
}
/// One immutable prepared page, including the original native text revisions.
pub struct Page<'a> {
    /// Validated shared renderer output and its native storage admission.
    pub frame: &'a PreparedFrame,
    /// Whole-window peers.
    pub shell: &'a [Control],
    /// Page peers relative to the viewport, already displaced by the requested scroll position.
    pub controls: &'a [Control],
    /// Logical caption region supplied by the same surface layout as the prepared branding.
    pub drag_bounds: Bounds,
    /// Logical page viewport; native peers must obey the same clip as painted content.
    pub viewport: Bounds,
    /// Actual clamped page offset in logical pixels.
    pub scroll: f32,
    /// Full page height before viewport clipping.
    pub content_height: f32,
}
#[cfg(windows)]
impl Page<'_> {
    /// Caption hit testing consumes prepared layout instead of a second adapter-owned header size.
    fn draggable(&self, x: f64, y: f64) -> bool {
        let b = self.drag_bounds;
        x >= f64::from(b.x)
            && x < f64::from(b.x + b.width)
            && y >= f64::from(b.y)
            && y < f64::from(b.y + b.height)
    }
}
/// Native input observations; the application decides whether the current page admits them.
pub enum Event {
    /// Invoke or observe a value from its original peer.
    Control(ControlId, Input),
    /// Current pointer/focus presentation facts, independent of navigation authority.
    Interaction {
        /// Enabled peer under the pointer.
        hovered: Option<ControlId>,
        /// Original peer retaining the current press gesture.
        pressed: Option<ControlId>,
        /// Original native peer holding keyboard focus.
        focused: Option<ControlId>,
    },
    /// Requested logical page offset; composition clamps against measured content.
    Scroll(f32),
    /// Result of the exact originally requested modal picker; cancellation preserves the old value.
    Picked(Picker, Result<Option<String>, String>),
}
/// Application lifetime polled on the native UI thread; no method waits for background work.
pub trait Lifetime {
    /// Stable peer inventory before native construction. Hidden peers retain their role and ID.
    fn inventory(&self) -> &[Control];
    /// Take one application-admitted path picker; the original request remains correlated until reply.
    fn picker(&mut self) -> Option<Picker>;
    /// Borrow prepared output from the shared native transcript owner.
    fn console(&self) -> Option<super::InstallerConsole<'_>>;
    /// Start admitted work with the adapter's completion wake.
    /// # Errors
    /// Retains partial execution ownership on failure; close/advance/settled/finish still apply.
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>>;
    /// Invalidate and coalesce a changed viewport before any old frame is accepted.
    /// # Errors
    /// Preserves geometry and execution admission failures.
    fn resize(&mut self, shape: FrameShape) -> Result<(), Box<dyn Error>>;
    /// Consume one typed native observation without starting work inside a window procedure.
    /// # Errors
    /// Preserves actual lifecycle failures; stale or unavailable user actions are handled by the flow.
    fn input(&mut self, event: Event) -> Result<(), Box<dyn Error>>;
    /// Drain actual backend/render work, including during close.
    /// # Errors
    /// Preserves the original execution or backend cause.
    fn advance(&mut self, elapsed: Duration) -> Result<(), Box<dyn Error>>;
    /// Borrow the latest accepted frame; its original metadata must remain stable for this borrow.
    fn page(&self) -> Option<Page<'_>>;
    /// Observe complete native presentation of the same immutable page.
    /// # Errors
    /// Preserves acknowledgement failures without granting process/storage authority.
    fn presented(&mut self) -> Result<(), Box<dyn Error>>;
    /// Lend an optional offscreen observer after actual native presentation has completed.
    #[cfg(any(test, feature = "boundary-fixtures"))]
    fn pixel_capture(&mut self) -> Option<&mut dyn crate::pixel_capture::PixelCapture> {
        None
    }
    /// Application-owned close request, such as successful launch of the installed application.
    fn close_requested(&self) -> bool;
    /// Seal admission and request bounded cleanup without joining active work.
    /// # Errors
    /// Retains partial ownership so subsequent advancement can settle it.
    fn close(&mut self) -> Result<(), Box<dyn Error>>;
    /// Actual worker/job settlement; cancellation alone is insufficient.
    /// # Errors
    /// Preserves unresolved cleanup causes.
    fn settled(&mut self) -> Result<bool, Box<dyn Error>>;
    /// Release final ownership after all native resources have retired on their creating thread.
    /// # Errors
    /// Preserves original final cleanup failures.
    fn finish(&mut self) -> Result<(), Box<dyn Error>>;
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

/// Reserve the native backing image, bounded field documents/providers, and copied peer values.
/// Raster preparation and application metadata require their own original reservations.
/// # Errors
/// Rejects an inventory larger than the shared native form bound.
pub fn storage_bytes(shape: FrameShape, fields: usize) -> Result<u64, Box<dyn Error>> {
    if fields > 64 {
        return Err("Native field inventory exceeds its bound".into());
    }
    let line = (19.0 * shape.scale()).ceil() as u64 + 2;
    let gutter = (f64::from(sugarsubstitute_ui::form::SCROLL_GUTTER) * shape.scale()).ceil() as u64;
    Ok(2 * gutter * u64::from(shape.height()) * 4
        + shape.byte_len() as u64
        + fields as u64
            * (2 * u64::from(shape.width()) * line * 4
                + 2 * 1024 * 1024
                + sugarsubstitute_contracts::presentation::NATIVE_CONSOLE_ACCESSIBILITY_BYTES)
        + 2 * 1024 * 1024)
}

/// Run the original native installer window through checked presentation and retirement.
/// # Errors
/// Preserves unsupported-platform, construction, application and native retirement failures.
pub fn run(options: Window, lifetime: &mut impl Lifetime) -> Result<(), Box<dyn Error>> {
    #[cfg(windows)]
    {
        windows::run(options, lifetime)
    }
    #[cfg(not(windows))]
    {
        let _ = (options, lifetime);
        Err("Native installer window is unavailable on this platform".into())
    }
}

/// Native shell selection kind; filesystem validation remains with installation ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerKind {
    /// Choose an existing directory.
    Folder,
    /// Choose an existing interpreter executable.
    File,
}
/// Original modal picker request, independent of display strings and filesystem authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Picker {
    /// Target path peer, never the Browse button itself.
    pub control: ControlId,
    /// Surface-local monotonically increasing request identity.
    pub request: u64,
    /// Native chooser semantics.
    pub kind: PickerKind,
}
