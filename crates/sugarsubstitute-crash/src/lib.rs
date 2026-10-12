// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own supervisor capture independently of desktop/backend cleanup families.
//!
//! Finite bridge calls run on the execution adapter. Cancelling their job never
//! proves native effects stopped or disposes their retained partial outputs.
//! Crashpad callbacks and their loaded module stay pinned until process exit.
//! Desktop connection, incident retention and reporting are separate owners.

#[cfg(windows)]
mod capture;
mod error;
mod input;
#[cfg(windows)]
#[allow(unsafe_code)]
mod native;
#[cfg(windows)]
pub use capture::{CaptureSnapshot, CaptureUpdates, StartRejected, WindowsCrashCapture};
pub use error::{CrashAdmissionError, CrashError};
pub use input::CaptureStartInput;

pub mod incident;
