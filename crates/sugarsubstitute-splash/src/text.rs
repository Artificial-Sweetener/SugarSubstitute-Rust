// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Prepare platform status glyphs after the first splash submission.

#[cfg(not(target_os = "windows"))]
#[path = "text_portable.rs"]
mod portable;

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
#[path = "text_windows.rs"]
mod native;

#[cfg(target_os = "windows")]
pub(crate) use native::StatusText;
#[cfg(not(target_os = "windows"))]
pub(crate) use portable::StatusText;
