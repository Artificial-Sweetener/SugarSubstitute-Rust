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
#[cfg(windows)]
mod capture_endpoint;
mod error;
mod input;
#[cfg(windows)]
#[allow(unsafe_code)]
mod native;
#[cfg(windows)]
pub use capture::{
    CaptureBootstrapRejection, CaptureBuildIdentity, CaptureCreator, CaptureSnapshot,
    CaptureUpdates, StartRejected, WindowsCrashCapture,
};
pub use error::{CrashAdmissionError, CrashError};
pub use input::CaptureStartInput;
#[cfg(windows)]
pub use native::desktop_driver::{DesktopCaptureDriverAlreadyCreated, desktop_capture_driver};

pub mod incident;
/// Format bounded charged redacted incident facts for native presentation.
pub mod report_view;

/// Bounded diagnostic schema1 frames, replay and actual transport loan routing.
pub mod diagnostic_client;
