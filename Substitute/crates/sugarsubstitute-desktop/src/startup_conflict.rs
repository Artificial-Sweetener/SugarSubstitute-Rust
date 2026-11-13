// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Ask whether to close an unresponsive previous window through the shared report surface
//! while the conflict controller owns process authority and its asynchronous native work.
use crate::{
    DesktopError,
    report::surface::{ConflictReport, ReportPresentation, show_report},
};
use sugarsubstitute_contracts::{instance_conflict::ConflictController, localization::Messages};

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
