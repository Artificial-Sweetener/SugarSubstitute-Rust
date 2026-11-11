// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own native window lifetime and CPU frame presentation independently of UI content.

#[cfg(target_os = "windows")]
mod bootstrap;
/// Closed schema-one facts about refused bulk channel requests; these grant no capability.
pub mod bulk_channel_diagnostic;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_activation;
#[cfg(target_os = "windows")]
pub use windows_activation::activate_desktop_process;
#[allow(unsafe_code)]
mod file_dialog;
pub use file_dialog::SelectionKind;
mod cadence;
/// Native canvas window/input lifetime with borrowed immutable prepared presentation.
pub mod canvas;
mod completion_wake;
mod console;
/// Closed console failure operation and direct cause codes; these grant no native authority.
pub mod console_failure_diagnostic;
mod contract;
pub use console::{ConsoleText, ConsoleView};
#[cfg(target_os = "windows")]
mod control_geometry;
mod frame_audit;
/// Closed schema-one frontend probe observations; these fields grant no readiness authority.
pub mod frontend_probe_diagnostic;
#[cfg(target_os = "windows")]
mod headless;
mod host_contract;
#[cfg(any(target_os = "windows", test))]
mod main_backend;
#[cfg(any(target_os = "windows", test))]
mod main_bulk;
mod main_canvas_native;
#[cfg(any(target_os = "windows", test))]
mod main_downloads;
mod main_environment;
mod main_hosting;
mod main_memory_contract;
mod main_placement;
pub use main_placement::MainPlacementObservation;
#[cfg(target_os = "windows")]
mod main_native_delivery;
mod main_navigation;
mod main_panel_contract;
#[cfg(any(target_os = "windows", test))]
mod main_panel_ingress;
mod main_panel_ingress_receipt;
#[cfg(test)]
mod main_panel_ingress_vectors_tests;
mod main_readiness;
mod main_readiness_storage;
mod main_readiness_types;
#[cfg(any(target_os = "windows", test))]
mod main_reload;
mod main_session_contract;
#[cfg(any(windows, test))]
mod main_session_ingress;
mod main_session_media;
mod main_surface_contract;
/// Bounded observational development trace records; feature-off builds expose none.
#[cfg(feature = "k1-native-qualification")]
pub mod qualification_trace;
#[cfg(all(target_os = "windows", feature = "k1-native-qualification"))]
mod qualified_preview;
#[cfg(target_os = "windows")]
pub use headless::run_headless;
mod headless_contract;
pub use headless_contract::{HeadlessLifetime, HeadlessStep};
#[cfg(all(target_os = "windows", feature = "k1-native-qualification"))]
pub use qualified_preview::{
    CapturedMainPreview, MainDownloadFixture, MainPreviewSnapshot, MainPreviewViewport,
};
/// Stable native setup window and its application-owned installation lifecycle.
pub mod installer;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod main_actor;
#[cfg(any(target_os = "windows", test))]
mod main_console;
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
#[cfg(target_os = "windows")]
mod startup_error_dialog;
mod startup_failure;
pub use startup_consume_failure::{
    StartupChildFailure, StartupChildStep, StartupConsumeFailure, StartupConsumeStage,
};
#[cfg(target_os = "windows")]
pub use startup_error_dialog::show_startup_failure;
mod timing;
pub use startup_failure::{
    StartupBootstrapCause, StartupCanvasMessageCause, StartupCaptureState, StartupCauseCode,
    StartupChallengeCause, StartupCloseFailure, StartupCloseStage, StartupNativeCause,
    StartupObservationKind, StartupOutputCause, StartupPipeCause, StartupRequestCause,
    StartupTimeCause,
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
mod windows_alpha;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_callbacks;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_clipboard;
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
mod windows_scroll;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_subclass;
#[cfg(target_os = "windows")]
mod windows_system_accent;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_text;

pub use contract::{
    AccessibleControl, ControlBounds, DesktopError, DesktopOptions, DesktopReceipt,
    FrameAuditReceipt, HitTarget, KeyAction, MovePresentation, NativeMaterial, Pointer,
    PointerAction, WindowContent,
};
pub use runtime::run;

#[cfg(any(test, feature = "boundary-fixtures"))]
pub mod pixel_capture;

pub use host_contract::{
    DesktopStartupCommand, DesktopStartupObservation, MainBackendState, MainCaptionLabels,
    MainIdentity, MainRecoveryLabels, MainReloadReason, MainRequestError, MainRetirementReason,
    MainWindowRequest, MainWindowSettings, ServedFrontendVersions, StartupDesktopReceipt,
    StartupPreparationStep, StartupWindowContent,
};

pub use main_canvas_native::{
    CanvasNativeNotification, CanvasNativeObservation, CanvasNativeReply, CanvasNativeReplyError,
    CanvasNativeRequestStorage,
};
pub use main_environment::{MainEnvironmentError, MainEnvironmentFacts};
pub use main_hosting::MainHosting;
pub use main_memory_contract::{MemoryCommand, MemoryObservation};
pub use main_panel_contract::{PanelCommand, PanelObservation};
pub use main_panel_ingress_receipt::{PanelIngressReceipt, PanelIngressReceiptError};
pub use main_session_contract::{SessionBootstrap, SessionCommand, SessionObservation};
pub use main_session_media::{
    MAX_MEDIA_BYTES, SessionMediaFailure, SessionMediaObservation, SessionMediaRange,
    SessionMediaReply,
};
pub use main_surface_contract::{
    SurfaceObservation, SurfacePresentation, SurfaceReply, SurfaceReplyError,
};

#[cfg(target_os = "windows")]
mod startup_conflict;
#[cfg(target_os = "windows")]
pub use startup_conflict::show_startup_conflict;

#[cfg(all(any(test, feature = "boundary-fixtures"), target_os = "windows"))]
#[allow(unsafe_code)]
mod windows_test_desktop;

/// Run one host libtest body on an owned private desktop and prove child-process termination.
/// Available only to explicit boundary qualification builds; never switches the input desktop.
/// # Errors
/// Preserves wrong test identity, native setup, child failure and checked retirement failures.
#[cfg(all(windows, feature = "boundary-fixtures"))]
pub fn qualify_installer_test(
    name: &'static str,
    body: fn() -> Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    windows_test_desktop::host(name, body)
}
