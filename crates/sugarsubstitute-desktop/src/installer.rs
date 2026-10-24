// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep one branded installer window alive through preparation, progress and cleanup.
use std::{error::Error, path::PathBuf, sync::Arc};
use sugarsubstitute_contracts::execution::CompletionWake;
use sugarsubstitute_contracts::presentation::PreparedFrame;

/// Actual native window dimensions and the local document's console placement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InstallerConsoleLayout {
    /// Physical window width.
    pub width: u32,
    /// Physical window height.
    pub height: u32,
    /// Native display scale.
    pub scale: f64,
    /// Logical console rectangle inside the window.
    pub bounds: crate::ControlBounds,
    /// Whether the user has revealed output.
    pub visible: bool,
}

/// Borrow the same native console contract used by the splash.
pub struct InstallerConsole<'a> {
    /// Prepared background and its admitted native storage.
    pub frame: &'a PreparedFrame,
    /// Borrowed immutable transcript and current layout.
    pub view: crate::ConsoleView<'a>,
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

/// Immutable setup presentation; HTML must be authored local content with a restrictive CSP.
pub struct InstallerWindow {
    /// Reveal the native window. Off-screen qualification keeps the same window hidden.
    pub visible: bool,
    /// Localized native title.
    pub title: String,
    /// Complete embedded document; no external navigation or network is admitted.
    pub html: String,
    /// Isolated webview data folder owned by this installation attempt.
    pub profile: PathBuf,
}

/// Application state owner polled by the native event loop without blocking it.
pub trait InstallerLifetime {
    /// Verify an observed browser profile off the UI thread before controller creation.
    /// # Errors
    /// Rejects duplicate or invalid environment admission.
    fn environment_observed(&mut self, path: PathBuf) -> Result<(), Box<dyn Error>>;
    /// Report possession of the actual verified profile identity and live exclusion.
    fn environment_ready(&self) -> bool;
    /// Request preparation for a validated placement without waiting on the UI thread.
    /// # Errors
    /// Rejects invalid native dimensions or unavailable preparation admission.
    fn console_layout(&mut self, layout: InstallerConsoleLayout) -> Result<(), Box<dyn Error>>;
    /// Borrow the latest prepared native console; stale layout work is never published.
    fn console(&self) -> Option<InstallerConsole<'_>>;
    /// Start admitted preparation after the native event loop provides its wake port.
    /// # Errors
    /// Returns the original execution/admission cause while retaining partial ownership.
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>>;
    /// Consume one bounded request from the local installer document.
    /// # Errors
    /// Returns lifecycle failures; ordinary input errors should be returned as UI replies.
    fn request(&mut self, json: &str) -> Result<(), Box<dyn Error>>;
    /// Progress owned work and return at most sixteen bounded JSON events for the document.
    /// # Errors
    /// Preserves original process, provisioning and storage failures.
    fn advance(&mut self) -> Result<Vec<String>, Box<dyn Error>>;
    /// Close admission and begin cleanup without waiting for it.
    /// # Errors
    /// Keeps partial process/job ownership on failure.
    fn close(&mut self) -> Result<(), Box<dyn Error>>;
    /// Report actual process and worker settlement, not a requested cancellation.
    /// # Errors
    /// Reports unresolved cleanup without pretending resources have retired.
    fn settled(&mut self) -> Result<bool, Box<dyn Error>>;
    /// Join already-settled execution resources after native teardown.
    /// # Errors
    /// Preserves final execution retirement failures.
    fn finish(&mut self) -> Result<(), Box<dyn Error>>;
}

/// Run the installer on the platform's native main thread.
/// # Errors
/// Returns unsupported-platform, native setup or retained application failures.
pub fn run(
    options: InstallerWindow,
    lifetime: &mut impl InstallerLifetime,
) -> Result<(), Box<dyn Error>> {
    #[cfg(windows)]
    {
        windows::run(options, lifetime)
    }
    #[cfg(not(windows))]
    {
        let _ = (options, lifetime);
        Err("The native installer adapter is not available on this platform".into())
    }
}
