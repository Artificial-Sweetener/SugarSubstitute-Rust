// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present terminal setup failures through the shared native report surface.
use crate::failure_details::Details;
use std::{error::Error, fmt};

/// Qualification and surface exports remain off-screen; interactive failures retain their cause.
pub(super) fn report<E: Error + 'static>(
    result: Result<(), Box<dyn Error>>,
    locale: &str,
    interactive: bool,
    show: impl FnOnce(
        &'static sugarsubstitute_contracts::localization::Messages,
        &str,
        &str,
        fmt::Arguments<'_>,
    ) -> Result<(), E>,
) -> Result<(), Box<dyn Error>> {
    let Err(cause) = result else {
        return Ok(());
    };
    tracing::error!(operation="installer_failed", cause_chain=%crate::diagnostics::CauseChain(cause.as_ref()), "Setup could not finish");
    if interactive {
        let copy = sugarsubstitute_contracts::localization::installer::messages(locale);
        if let Err(secondary) = show(
            sugarsubstitute_contracts::localization::messages(locale),
            copy.title,
            copy.setup_failed,
            format_args!("{}", Details(cause.as_ref())),
        ) {
            tracing::error!(operation="installer_failure_presentation", cause=?secondary,
                "Native setup failure presentation failed; retaining the original failure");
            return Err(crate::failure_details::retain_presentation(cause, secondary));
        }
    }
    Err(cause)
}

/// Present typed recovery decisions through installer localization while retaining diagnostics.
pub(super) fn storage_message(cause: &std::io::Error, locale: &str) -> String {
    use sugarsubstitute_supervision::setup_storage::RecoveryFailure;
    let copy = sugarsubstitute_contracts::localization::installer::messages(locale);
    if cause.get_ref().is_some_and(|cause| {
        cause.is::<super::controller::requests::metadata::IncludedVersionOffered>()
    }) {
        return copy.included_version_offer.to_owned();
    }
    match cause.get_ref().and_then(|cause| cause.downcast_ref::<RecoveryFailure>()) {
        Some(RecoveryFailure::ConflictingSettings) => copy.recovery_conflict,
        Some(RecoveryFailure::IncompleteSettings) => copy.recovery_incomplete,
        Some(RecoveryFailure::InventoryBound) => copy.recovery_inventory_bound,
        None => copy.setup_failed,
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn typed_recovery_failures_are_localized_without_losing_original_diagnostics() {
        use sugarsubstitute_supervision::setup_storage::RecoveryFailure;
        for reason in [
            RecoveryFailure::ConflictingSettings,
            RecoveryFailure::IncompleteSettings,
            RecoveryFailure::InventoryBound,
        ] {
            let cause = io::Error::other(reason);
            for locale in sugarsubstitute_contracts::localization::LOCALES {
                let expected = match reason {
                    RecoveryFailure::ConflictingSettings => locale.installer.recovery_conflict,
                    RecoveryFailure::IncompleteSettings => locale.installer.recovery_incomplete,
                    RecoveryFailure::InventoryBound => locale.installer.recovery_inventory_bound,
                };
                assert_eq!(storage_message(&cause, locale.id), expected);
            }
            assert!(Details(&cause).to_string().contains(&format!("{reason:?}")));
        }
        let other = io::Error::other("original filesystem cause");
        assert_eq!(
            storage_message(&other, "ja"),
            sugarsubstitute_contracts::localization::installer::messages("ja").setup_failed
        );
    }

    #[test]
    fn early_failure_is_visible_once_and_preserves_its_original_cause() {
        let original = io::Error::new(io::ErrorKind::PermissionDenied, "package unreadable");
        let mut shown = 0;
        let result = report(Err(Box::new(original)), "en", true, |_, title, summary, details| {
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
            let outcome = report(result, "ja", false, |_, _, _, _| {
                shown += 1;
                Ok::<_, io::Error>(())
            });
            assert_eq!(outcome.is_err(), failed);
        }
        assert!(
            report(Ok(()), "ko", true, |_, _, _, _| {
                shown += 1;
                Ok::<_, io::Error>(())
            })
            .is_ok()
        );
        assert_eq!(shown, 0);
    }

    #[test]
    fn dialog_failure_does_not_replace_the_installation_failure() {
        let result = report(Err("original install failure".into()), "es", true, |_, _, _, _| {
            Err(io::Error::other("dialog unavailable"))
        });
        assert!(result.is_err_and(|cause| cause.to_string() == "original install failure"));
    }

    #[test]
    fn terminal_dialog_keeps_the_selected_installer_locale() {
        for locale in sugarsubstitute_contracts::localization::LOCALES {
            let result = report(
                Err("Original setup diagnostic".into()),
                locale.id,
                true,
                |_, title, summary, details| {
                    assert_eq!(title, locale.installer.title);
                    assert_eq!(summary, locale.installer.setup_failed);
                    assert_eq!(details.to_string(), "Original setup diagnostic");
                    Ok::<_, io::Error>(())
                },
            );
            assert!(result.is_err_and(|cause| cause.to_string() == "Original setup diagnostic"));
        }
    }
}
