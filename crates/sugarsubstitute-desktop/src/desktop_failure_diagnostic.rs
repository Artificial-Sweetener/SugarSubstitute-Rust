// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe the first retained native failure through closed metadata only.
use std::{error::Error, io::ErrorKind};

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
    let actual = classify(operation, cause);
    tracing::warn!(operation = "desktop_failure_transition",
        boundary = ?actual.boundary, kind = ?actual.kind,
        "First owned desktop failure retained");
}
#[cfg(test)]
#[path = "desktop_failure_diagnostic/tests.rs"]
mod tests;
