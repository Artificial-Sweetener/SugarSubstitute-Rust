// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Transfer immutable validated presentation pixels with their admitted memory.
//!
//! CPU preparation incrementally validates premultiplication before publication.
//! Native presentation consumes only immutable pixels and constant-time metadata;
//! it does not rescan alpha or take ownership of an execution scheduler.

mod frame;

pub use frame::{
    FrameError, FrameFormat, FrameIdentity, FrameShape, FrameValidator, PreparedFrame,
};

mod console;
pub use console::{ConsolePresentation, NATIVE_CONSOLE_STORAGE_BYTES, console_color};
