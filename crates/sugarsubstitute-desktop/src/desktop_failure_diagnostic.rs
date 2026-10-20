// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe the first retained native failure through closed metadata only.
use std::{error::Error, io::ErrorKind};
#[path = "desktop_failure_diagnostic/console.rs"]
mod console;
#[path = "desktop_failure_diagnostic/startup.rs"]
mod startup;

/// Project direct sealed native/CDP cause data without formatting opaque payloads.
#[cfg(any(target_os = "windows", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MainCode {
    #[cfg(target_os = "windows")]
    Hresult(i32),
    Os(i32),
    Io(Kind),
    Readiness(crate::main_readiness_types::ReadinessFault),
    JsonIo,
    JsonSyntax,
    JsonData,
    JsonEof,
    Opaque,
}

/// Inspect only known direct concrete causes; preserve the original error and all callbacks.
#[cfg(any(target_os = "windows", test))]
fn main_code(cause: &(dyn Error + 'static)) -> MainCode {
    #[cfg(target_os = "windows")]
    if let Some(native) = cause.downcast_ref::<windows::core::Error>() {
        return MainCode::Hresult(native.code().0);
    }
    if let Some(readiness) = cause.downcast_ref::<crate::main_readiness_types::ReadinessFault>() {
        return MainCode::Readiness(*readiness);
    }
    if let Some(io) = cause.downcast_ref::<std::io::Error>() {
        return io.raw_os_error().map_or_else(|| MainCode::Io(classify("", io).kind), MainCode::Os);
    }
    if let Some(json) = cause.downcast_ref::<serde_json::Error>() {
        return match json.classify() {
            serde_json::error::Category::Io => MainCode::JsonIo,
            serde_json::error::Category::Syntax => MainCode::JsonSyntax,
            serde_json::error::Category::Data => MainCode::JsonData,
            serde_json::error::Category::Eof => MainCode::JsonEof,
        };
    }
    MainCode::Opaque
}

#[cfg(any(target_os = "windows", test))]
impl super::DesktopError {
    /// Observe the exact owned main failure before its transfer to startup policy.
    /// Authored static operation labels and direct closed cause codes carry no native authority.
    /// This never formats the original cause or invokes its arbitrary source callback.
    pub(crate) fn observe_main_failure(
        &self,
        identity: crate::MainIdentity,
        observation: &'static str,
    ) {
        let code = main_code(self.cause.as_ref());
        tracing::warn!(operation = "main_failure_transition", failed_operation = self.operation,
            launch = identity.launch.get(), navigation = identity.navigation.get(),
            observation = %observation, code = ?code,
            "Original owned main failure transferred to startup policy");
    }
}

#[cfg(any(target_os = "windows", test))]
struct ProbeOperation(Option<u64>);
#[cfg(any(target_os = "windows", test))]
impl std::fmt::Display for ProbeOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(value) => write!(f, "{value}"),
            None => f.write_str("None"),
        }
    }
}
/// Emit the authoritative frontend expiry in the existing first Main failure frame.
/// Facts come from the actual creating-thread owner; no error callbacks or clock are invoked.
#[cfg(any(target_os = "windows", test))]
impl super::DesktopError {
    pub(crate) fn observe_frontend_timeout(
        identity: crate::MainIdentity,
        snapshot: crate::main_readiness::snapshot::Snapshot,
    ) {
        let operation = ProbeOperation(snapshot.operation);
        let observation = "None";
        tracing::warn!(operation="main_failure_transition", failed_operation="frontend_budget_expired",
        launch=identity.launch.get(), navigation=identity.navigation.get(),
        observation=%observation, code=?MainCode::Io(Kind::TimedOut),
        wait=?snapshot.wait, probe_operation=%operation,
        "Original frontend cutoff reached before readiness acceptance");
    }
}

/// Project only known native/content boundaries without copying arbitrary operation text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Boundary {
    PollFrame,
    RequestFrame,
    ObserveStartup,
    TakeCommand,
    Visibility,
    PreSurface,
    Cleanup,
    Presentation,
    CursorMonitor,
    MonitorWorkArea,
    CenterWindow,
    NativeFocus,
    NativeFocusRelease,
    ControlUpdate,
    FramePresented,
    Other,
}
/// Classify a directly retained IO cause without invoking its formatter or source callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    TimedOut,
    Disconnected,
    Denied,
    InvalidInput,
    Io,
    Other,
}
/// Copy bounded metadata while the authoritative DesktopError continues to own its cause.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Diagnostic {
    boundary: Boundary,
    kind: Kind,
}
/// Derive closed facts from the operation and direct type; unknown causes remain opaque.
fn classify(operation: &'static str, cause: &(dyn Error + 'static)) -> Diagnostic {
    let boundary = match operation {
        "poll_frame" => Boundary::PollFrame,
        "request_frame" => Boundary::RequestFrame,
        "observe_startup" => Boundary::ObserveStartup,
        "take_startup_command" => Boundary::TakeCommand,
        "set_content_visibility" => Boundary::Visibility,
        "startup_preparation" => Boundary::PreSurface,
        "hide_content"
        | "request_shutdown"
        | "poll_shutdown"
        | "finish_preparation_shutdown"
        | "finish_startup_shutdown" => Boundary::Cleanup,
        "present_startup_splash" => Boundary::Presentation,
        "query_cursor_monitor" => Boundary::CursorMonitor,
        "query_monitor_work_area" => Boundary::MonitorWorkArea,
        "center_window" => Boundary::CenterWindow,
        "draw_native_focus" => Boundary::NativeFocus,
        "release_native_focus_dc" => Boundary::NativeFocusRelease,
        "update_accessible_controls"
        | "validate_accessible_controls"
        | "validate_accessible_bounds" => Boundary::ControlUpdate,
        "frame_presented" => Boundary::FramePresented,
        _ => Boundary::Other,
    };
    let kind = match cause.downcast_ref::<std::io::Error>().map(std::io::Error::kind) {
        Some(ErrorKind::TimedOut) => Kind::TimedOut,
        Some(
            ErrorKind::BrokenPipe
            | ErrorKind::ConnectionAborted
            | ErrorKind::ConnectionReset
            | ErrorKind::NotConnected,
        ) => Kind::Disconnected,
        Some(ErrorKind::PermissionDenied) => Kind::Denied,
        Some(ErrorKind::InvalidData | ErrorKind::InvalidInput) => Kind::InvalidInput,
        Some(_) => Kind::Io,
        None => Kind::Other,
    };
    Diagnostic { boundary, kind }
}
/// Called only after actual DesktopError first-cause adoption. The cause is borrowed;
/// its formatter/source callbacks are never invoked and no native authority is projected.
pub(super) fn emit(operation: &'static str, cause: &(dyn Error + 'static)) {
    if startup::emit(operation, cause) || console::emit(operation, cause) {
        return;
    }
    let actual = classify(operation, cause);
    tracing::warn!(operation = "desktop_failure_transition",
        boundary = ?actual.boundary, kind = ?actual.kind,
        "First owned desktop failure retained");
}
#[cfg(test)]
#[path = "desktop_failure_diagnostic/tests.rs"]
mod tests;
