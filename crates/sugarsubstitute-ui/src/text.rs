// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Bounded platform label and console shaping shared by native surfaces.

#[path = "text_console.rs"]
pub mod console;
#[path = "text_layout.rs"]
mod layout;
pub use layout::{
    LabelLayout, LabelPaintSpec, PaintSpec, TextAlignment, TextMetrics, TextSize, VerticalAnchor,
};
#[cfg(test)]
#[path = "text_fixtures.rs"]
mod fixtures;
#[path = "text_memory.rs"]
mod memory;

#[cfg(not(target_os = "windows"))]
#[path = "text_portable.rs"]
mod portable;

// Exercise the portable glyph algorithm on Windows as well; this does not qualify
// native Linux/macOS font discovery or presentation.
#[cfg(all(test, target_os = "windows"))]
#[path = "text_portable.rs"]
pub mod portable_probe;

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
#[path = "text_windows.rs"]
mod native;

#[cfg(target_os = "windows")]
pub use native::{FontOwner, GlyphPaint};
#[cfg(not(target_os = "windows"))]
pub use portable::{FontOwner, GlyphPaint};
