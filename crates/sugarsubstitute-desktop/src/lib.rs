// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own native window lifetime and CPU frame presentation independently of UI content.

#[cfg(target_os = "windows")]
mod bootstrap;
mod cadence;
mod completion_wake;
mod contract;
#[cfg(target_os = "windows")]
mod control_geometry;
mod frame_audit;
#[cfg(target_os = "windows")]
mod headless;
mod host_contract;
mod main_environment;
mod main_navigation;
mod main_readiness;
mod main_readiness_storage;
mod main_readiness_types;
#[cfg(target_os = "windows")]
pub use headless::{HeadlessLifetime, HeadlessStep, run_headless};
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod main_actor;
#[cfg(target_os = "windows")]
mod main_events;
mod native_action;
#[cfg(any(target_os = "windows", target_os = "linux", test))]
#[allow(unsafe_code)]
mod native_storage;
#[allow(unsafe_code)]
mod native_surface;
/// Own one scoped native incident report surface independently of webviews.
pub mod report;
mod runtime;
#[path = "startup_custody.rs"]
mod shutdown;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod startup_runtime;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_main_receivers;
#[cfg(target_os = "windows")]
pub use startup_runtime::run_startup;
mod startup_consume_failure;
mod startup_failure;
pub use startup_consume_failure::{StartupConsumeFailure, StartupConsumeStage};
mod timing;
pub use startup_failure::{
    StartupBootstrapCause, StartupCaptureState, StartupCauseCode, StartupCloseFailure,
    StartupCloseStage, StartupNativeCause, StartupRequestCause,
};
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_animation_period;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_timing;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_ui_apartment;
pub use timing::{
    NativeCallbackKind, NativeCallbackTiming, NativeStartupOrder, NativeStartupStage,
    NativeStartupTiming, NativeSubmissionTiming, NativeTickInterval, NativeTimingReceipt,
    NativeTimingSource,
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

pub use host_contract::{
    DesktopStartupCommand, DesktopStartupObservation, MainCaptionLabels, MainIdentity,
    MainRequestError, MainRetirementReason, MainWindowRequest, MainWindowSettings,
    ServedFrontendVersions, StartupDesktopReceipt, StartupPreparationStep, StartupWindowContent,
};

pub use main_environment::{MainEnvironmentError, MainEnvironmentFacts};
