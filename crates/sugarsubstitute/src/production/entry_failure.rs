// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Report a failed application entry after its original cleanup lifetime.
use super::entry_runner::EntryActivation;
use std::error::Error;
use std::fmt;

/// Present actionable profile contention while retaining full diagnostics for other failures.
struct FailureDetails<'a>(&'a (dyn Error + 'static));

impl fmt::Display for FailureDetails<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if sugarsubstitute_supervision::profiles::ProfileInUse::in_error_chain(self.0) {
            f.write_str(sugarsubstitute_contracts::localization::messages("en").profile_in_use)
        } else {
            write!(f, "{:?}", super::entry_runner::FailureDisplay(self.0))
        }
    }
}

/// Present only this user-facing Root's failed lifetime, retaining its exact original error.
pub(super) fn report_failure<C: EntryActivation + 'static, E: Error>(
    cause: Box<dyn Error>,
    show: impl FnOnce(&str, &str, std::fmt::Arguments<'_>) -> Result<(), E>,
) -> Box<dyn Error> {
    if super::entry_runner::is_normal_shutdown_failure::<C>(cause.as_ref()) {
        tracing::error!(operation="application_shutdown_failed",
            cause=?super::entry_runner::FailureDisplay(cause.as_ref()),
            "User-requested shutdown finished with retained cleanup errors");
        return cause;
    }
    if let Err(secondary) = show(
        sugarsubstitute::APPLICATION_NAME,
        sugarsubstitute_contracts::localization::messages("en").report.startup_failed,
        format_args!("{}", FailureDetails(cause.as_ref())),
    ) {
        tracing::error!(operation="startup_failure_presentation",cause=?secondary,
            "Native startup failure presentation failed while its original application cause remains retained");
    }
    cause
}
