// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own native window lifetime and CPU frame presentation independently of UI content.

mod contract;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
mod runtime;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows;

pub use contract::{
    DesktopError, DesktopOptions, DesktopReceipt, Frame, HitTarget, KeyAction, NativeMaterial,
    Pointer, PointerAction, WindowContent,
};
pub use runtime::run;
