// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Report a failed application entry after its original cleanup lifetime.
use super::entry_runner::EntryActivation;
use std::error::Error;

/// Present only this user-facing Root's failed lifetime, retaining its exact original error.
/// A lifetime whose window had opened is reported through `show_running`, as the application
/// closing unexpectedly; every earlier failure is a startup failure through `show`.
pub(super) fn report_failure<C: EntryActivation + 'static, E: Error + 'static>(
    cause: Box<dyn Error>,
    show: impl FnOnce(
        &'static sugarsubstitute_contracts::localization::Messages,
        &str,
        &str,
        std::fmt::Arguments<'_>,
    ) -> Result<(), E>,
    show_running: impl FnOnce(
        &'static sugarsubstitute_contracts::localization::Messages,
        &str,
        &str,
        std::fmt::Arguments<'_>,
    ) -> Result<(), E>,
) -> Box<dyn Error> {
    if super::entry_runner::is_normal_shutdown_failure::<C>(cause.as_ref()) {
        tracing::error!(operation="application_shutdown_failed",
            cause_chain=%crate::diagnostics::CauseChain(cause.as_ref()),
            "User-requested shutdown finished with retained cleanup errors");
        return cause;
    }
    let copy = super::entry_runner::presentation::locale::<C>(cause.as_ref());
    let in_use =
        sugarsubstitute_supervision::profiles::ProfileInUse::in_error_chain(cause.as_ref());
    let running = !in_use && super::entry_runner::presentation::window_opened::<C>(cause.as_ref());
    let details = super::entry_runner::FailureDisplay(cause.as_ref());
    let shown = if running {
        show_running(
            copy,
            sugarsubstitute::APPLICATION_NAME,
            copy.report.title,
            format_args!("{details:?}"),
        )
    } else {
        show(
            copy,
            sugarsubstitute::APPLICATION_NAME,
            if in_use { copy.profile_in_use } else { copy.report.startup_failed },
            format_args!("{details:?}"),
        )
    };
    if let Err(secondary) = shown {
        tracing::error!(operation="startup_failure_presentation",cause=?secondary,
            "Native startup failure presentation failed while its original application cause remains retained");
        return crate::failure_details::retain_presentation(cause, secondary);
    }
    cause
}
