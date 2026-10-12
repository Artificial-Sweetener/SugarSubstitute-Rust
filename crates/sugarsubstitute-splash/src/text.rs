// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Own bounded platform status and console shaping after the first splash submission.

#[path = "text_console.rs"]
pub(crate) mod console;
#[path = "text_memory.rs"]
mod memory;

#[cfg(not(target_os = "windows"))]
#[path = "text_portable.rs"]
mod portable;

// Exercise the portable glyph algorithm on Windows as well; this does not qualify
// native Linux/macOS font discovery or presentation.
#[cfg(all(test, target_os = "windows"))]
#[path = "text_portable.rs"]
mod portable_probe;

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
#[path = "text_windows.rs"]
mod native;

#[cfg(target_os = "windows")]
pub(crate) use native::{FontOwner, GlyphPaint};
#[cfg(not(target_os = "windows"))]
pub(crate) use portable::{FontOwner, GlyphPaint};
