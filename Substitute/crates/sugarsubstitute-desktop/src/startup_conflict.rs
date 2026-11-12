// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Ask whether to close an unresponsive previous window through the shared report surface
//! while the conflict controller owns process authority and its asynchronous native work.
use crate::{
    DesktopError,
    report::surface::{ConflictReport, ReportPresentation, show_report},
};
use sugarsubstitute_contracts::{instance_conflict::ConflictController, localization::Messages};
use sugarsubstitute_ui::report::{ReportContent, ReportKind, ReportSeverity};

/// Ask until the controller has actually retired its work. "Close it and start" is offered
/// while the controller can act on the previous window; Cancel, the window's close button
/// and Escape cancel. Must not be called from inside a window event callback.
/// # Errors
/// Native window failure is returned; the caller must still retire its controller separately.
pub fn show_startup_conflict(
    copy: &'static Messages,
    title: &str,
    controller: &mut dyn ConflictController,
) -> Result<(), DesktopError> {
    show_report(
        ReportPresentation::unresponsive_copy(copy, title),
        &mut ConflictReport::new(controller, copy.conflict),
    )
    .map_err(|cause| DesktopError::boxed("profile_conflict_report", cause))
}

/// Wait on screen while the previous window finishes closing, then let this launch continue.
/// Only Cancel is offered; if the previous window stops responding meanwhile, the same surface
/// asks whether to close it and start. Must not be called from inside a window event callback.
/// # Errors
/// Native window failure is returned; the caller must still retire its controller separately.
pub fn show_previous_copy_closing(
    copy: &'static Messages,
    title: &str,
    controller: &mut dyn ConflictController,
) -> Result<(), DesktopError> {
    let content = ReportContent {
        kind: ReportKind::UnresponsiveCopy,
        severity: ReportSeverity::Info,
        window_title: title.to_owned(),
        title: copy.conflict.starting.to_owned(),
        message: copy.conflict.cleanup.to_owned(),
        summary: Vec::new(),
        has_details: false,
    };
    show_report(
        ReportPresentation::new(content, None, copy),
        &mut ConflictReport::new(controller, copy.conflict),
    )
    .map_err(|cause| DesktopError::boxed("previous_copy_closing_report", cause))
}
