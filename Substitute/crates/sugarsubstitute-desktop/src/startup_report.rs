// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The update report opened over the running main window: a hosted form window on the
//! application's own event loop, owned by and centred on the main window, showing the same
//! report view as the standalone surfaces. It offers Copy and Export; nothing restarts.
use super::*;
use crate::{
    installer::native::{FormOwner, HostedForm},
    native_action::{FormAction, FormScope},
    report::surface::{ReportPresentation, UpdateExport, UpdateReport, lifetime::ReportLifetime},
};
use sugarsubstitute_contracts::update::status::UpdateStatus;
use tao::{event::WindowEvent, window::WindowId};

/// The hosted update report.
pub(in crate::startup_runtime) type HostedReport = HostedForm<ReportLifetime<'static>>;

/// The report's lifetime, exporting through `export` when the host has a writer.
fn lifetime(
    presentation: ReportPresentation,
    export: Option<Box<dyn UpdateExport>>,
) -> Result<ReportLifetime<'static>, Box<dyn std::error::Error>> {
    let owner = UpdateReport::new(export, &presentation);
    ReportLifetime::new(presentation, owner)
}

/// Open the report of the latest failed application update and SugarCubes update over the
/// main window, or bring an open one forward. Does nothing when nothing failed or the main
/// window is not up.
pub(super) fn open(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
    target: &EventLoopWindowTarget<NativeAction>,
    proxy: EventLoopProxy<NativeAction>,
    title: &str,
) -> Result<(), DesktopError> {
    if let Some(report) = &session.report {
        report.focus();
        return Ok(());
    }
    let Some(window) = session.main.as_ref().and_then(MainActor::activity_window) else {
        return Ok(());
    };
    use tao::platform::windows::WindowExtWindows;
    let owner = FormOwner(::windows::Win32::Foundation::HWND(window.hwnd() as *mut _));
    let words = content.locale_messages();
    let status = content.update_status();
    let app = status.as_ref().and_then(UpdateStatus::failure);
    let failed =
        ReportPresentation::failed_updates(words, title, app, content.pack_update_failure());
    let Some(presentation) = failed else { return Ok(()) };
    let picker = session
        .picker
        .take()
        .ok_or_else(|| DesktopError::message("open_update_report", "shell picker is held"))?;
    let opened = lifetime(presentation, content.update_report_export()).and_then(|lifetime| {
        let window = crate::report::surface::report_window(title.to_owned());
        HostedForm::open(target, proxy, picker.clone(), owner, window, lifetime)
    });
    match opened {
        Ok(report) => session.report = Some(report),
        Err(cause) => {
            session.picker = Some(picker);
            tracing::error!(operation = "open_update_report", cause = %cause,
                "The update report could not open");
        }
    }
    Ok(())
}

/// Whether `window` is the report's window.
pub(super) fn owns_window(session: &Session, window: WindowId) -> bool {
    session.report.as_ref().is_some_and(|report| report.owns_window(window))
}

/// Deliver a window event addressed to the report's window.
pub(super) fn window_event(session: &mut Session, event: &WindowEvent<'_>) {
    if let Some(report) = &mut session.report {
        report.window_event(event);
    }
}

/// Deliver an action from the report's peers.
pub(super) fn action(session: &mut Session, scope: FormScope, action: FormAction) {
    if let Some(report) = &mut session.report {
        report.action(scope, action);
    }
}

/// Repaint the report's window.
pub(super) fn redraw(session: &mut Session) {
    if let Some(report) = &mut session.report {
        report.redraw();
    }
}

/// Deliver a changed system accent.
pub(super) fn accent(
    session: &mut Session,
    accent: Option<sugarsubstitute_contracts::appearance::AccentColor>,
) {
    if let Some(report) = &mut session.report {
        report.accent(accent);
    }
}

/// Advance the report one turn. It closes when the application or the main window closes, and
/// the main window is not destroyed while the report's window exists.
pub(super) fn turn(session: &mut Session) {
    let closing = session.closing || session.main.as_ref().is_none_or(MainActor::closing);
    let mut holds_owner = false;
    if let Some(report) = &mut session.report {
        let (turn, picker) = report.turn(closing);
        if picker.is_some() {
            session.picker = picker;
        }
        holds_owner = turn.holds_owner;
        if turn.finished {
            session.report = None;
        }
    }
    if let Some(main) = &mut session.main {
        main.hold_for_owned_window(holds_owner);
    }
}
