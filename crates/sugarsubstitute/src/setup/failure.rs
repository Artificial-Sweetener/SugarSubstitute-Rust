// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present terminal setup failures through the existing native failure dialog.
use crate::failure_details::Details;
use std::{error::Error, fmt};

/// Qualification and surface exports remain off-screen; interactive failures retain their cause.
pub(super) fn report<E: Error>(
    result: Result<(), Box<dyn Error>>,
    interactive: bool,
    show: impl FnOnce(&str, &str, fmt::Arguments<'_>) -> Result<(), E>,
) -> Result<(), Box<dyn Error>> {
    let Err(cause) = result else {
        return Ok(());
    };
    tracing::error!(operation="installer_failed", cause=%Details(cause.as_ref()), "Setup could not finish");
    if interactive {
        let copy = sugarsubstitute_contracts::localization::installer::messages("en");
        if let Err(secondary) =
            show(copy["title"], copy["setupFailed"], format_args!("{}", Details(cause.as_ref())))
        {
            tracing::error!(operation="installer_failure_presentation", cause=?secondary,
                "Native setup failure presentation failed; retaining the original failure");
        }
    }
    Err(cause)
}

/// Present typed recovery decisions through installer localization while retaining diagnostics.
pub(super) fn storage_message(cause: &std::io::Error, locale: &str) -> String {
    use sugarsubstitute_supervision::setup_storage::RecoveryFailure;
    let key = match cause.get_ref().and_then(|cause| cause.downcast_ref::<RecoveryFailure>()) {
        Some(RecoveryFailure::ConflictingSettings) => "recoveryConflict",
        Some(RecoveryFailure::IncompleteSettings) => "recoveryIncomplete",
        Some(RecoveryFailure::InventoryBound) => "recoveryInventoryBound",
        None => return Details(cause).to_string(),
    };
    sugarsubstitute_contracts::localization::installer::messages(locale)[key].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn typed_recovery_failures_are_localized_without_losing_original_diagnostics() {
        use sugarsubstitute_supervision::setup_storage::RecoveryFailure;
        for (reason, key) in [
            (RecoveryFailure::ConflictingSettings, "recoveryConflict"),
            (RecoveryFailure::IncompleteSettings, "recoveryIncomplete"),
            (RecoveryFailure::InventoryBound, "recoveryInventoryBound"),
        ] {
            let cause = io::Error::other(reason);
            for locale in sugarsubstitute_contracts::localization::LOCALES {
                assert_eq!(
                    storage_message(&cause, locale.id),
                    sugarsubstitute_contracts::localization::installer::messages(locale.id)[key]
                );
            }
            assert!(Details(&cause).to_string().contains(&format!("{reason:?}")));
        }
        let other = io::Error::other("original filesystem cause");
        assert_eq!(storage_message(&other, "ja"), "original filesystem cause");
    }

    #[test]
    fn early_failure_is_visible_once_and_preserves_its_original_cause() {
        let original = io::Error::new(io::ErrorKind::PermissionDenied, "package unreadable");
        let mut shown = 0;
        let result = report(Err(Box::new(original)), true, |title, summary, details| {
            shown += 1;
            assert_eq!(title, "SugarSubstitute Setup");
            assert!(!summary.is_empty());
            assert_eq!(details.to_string(), "package unreadable");
            Ok::<_, io::Error>(())
        });
        assert_eq!(shown, 1);
        assert!(result.is_err_and(|cause| {
            cause
                .downcast_ref::<io::Error>()
                .is_some_and(|cause| cause.kind() == io::ErrorKind::PermissionDenied)
        }));
    }

    #[test]
    fn successful_close_and_offscreen_failures_never_open_a_dialog() {
        let mut shown = 0;
        for result in [Ok(()), Err(Box::new(io::Error::other("fixture")) as Box<dyn Error>)] {
            let failed = result.is_err();
            let outcome = report(result, false, |_, _, _| {
                shown += 1;
                Ok::<_, io::Error>(())
            });
            assert_eq!(outcome.is_err(), failed);
        }
        assert!(
            report(Ok(()), true, |_, _, _| {
                shown += 1;
                Ok::<_, io::Error>(())
            })
            .is_ok()
        );
        assert_eq!(shown, 0);
    }

    #[test]
    fn dialog_failure_does_not_replace_the_installation_failure() {
        let result = report(Err("original install failure".into()), true, |_, _, _| {
            Err(io::Error::other("dialog unavailable"))
        });
        assert!(result.is_err_and(|cause| cause.to_string() == "original install failure"));
    }
}
