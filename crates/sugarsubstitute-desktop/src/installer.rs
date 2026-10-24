// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep one branded installer window alive through preparation, progress and cleanup.
/// Typed native page presentation and input through the shared native control owner.
pub mod native;
use sugarsubstitute_contracts::presentation::PreparedFrame;

/// Actual native window dimensions and the prepared console placement.
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
    /// Original disclosure control receiving focus when the transcript is collapsed.
    pub focus_return: Option<sugarsubstitute_ui::form::ControlId>,
    /// Prepared background and its admitted native storage.
    pub frame: &'a PreparedFrame,
    /// Borrowed immutable transcript and current layout.
    pub view: crate::ConsoleView<'a>,
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod dialog;
