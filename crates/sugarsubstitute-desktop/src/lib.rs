// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own native window lifetime and CPU frame presentation independently of UI content.

mod cadence;
mod contract;
#[cfg(target_os = "windows")]
mod control_geometry;
mod frame_audit;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(target_os = "windows", target_os = "linux", test))]
#[allow(unsafe_code)]
mod native_storage;
mod runtime;
mod timing;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_animation_period;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_timing;
pub use timing::{
    NativeCallbackKind, NativeCallbackTiming, NativeSubmissionTiming, NativeTickInterval,
    NativeTimingReceipt, NativeTimingSource,
};
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_callbacks;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_controls;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_drag;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_move_state;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_subclass;

pub use contract::{
    AccessibleControl, ControlBounds, DesktopError, DesktopOptions, DesktopReceipt,
    FrameAuditReceipt, HitTarget, KeyAction, MovePresentation, NativeMaterial, Pointer,
    PointerAction, WindowContent,
};
pub use runtime::run;
