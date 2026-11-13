// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Borrow presentation from the retained original lifetime without a second configuration read.
use super::{EntryActivation, RunFailure};
use std::error::Error;
use sugarsubstitute_contracts::localization::{self, Messages};

/// Use the exact failed activation's selection, falling back before configuration was authenticated.
pub(in crate::production) fn locale<C: EntryActivation + 'static>(
    cause: &(dyn Error + 'static),
) -> &'static Messages {
    let mut current = Some(cause);
    for _ in 0..16 {
        let Some(cause) = current else {
            break;
        };
        if let Some(failure) = cause.downcast_ref::<RunFailure<C>>() {
            return localization::messages(
                failure.lifetime.child.presentation_locale().map_or("en", |locale| locale.as_str()),
            );
        }
        current = cause.source();
    }
    localization::messages("en")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::production::{
        dispatch::ProductionCommand,
        entry_runner::{
            EntryRunner,
            tests::{NoChild, unopened_paths},
        },
    };
    use std::path::PathBuf;

    #[test]
    fn retained_failure_uses_its_own_authenticated_locale_and_preserves_original_cause()
    -> Result<(), Box<dyn Error>> {
        assert_eq!(locale::<NoChild>(&std::io::Error::other("Before configuration")).id, "en");
        for selected in localization::LOCALES {
            let path = PathBuf::from("unopened-input");
            let command = ProductionCommand {
                launch_selection: None,
                installation: path.clone(),
                diagnostics: Default::default(),
                options: unopened_paths()?.into(),
            };
            let child = NoChild(Some(sugarsubstitute_contracts::configuration::LocaleId::new(
                selected.id,
            )?));
            let lifetime = Box::new(EntryRunner::new(
                command,
                path,
                Box::new(child),
                crate::diagnostics::DiagnosticHealth::fixture(),
            ));
            let failure: Box<dyn Error> =
                Box::new(RunFailure { pump: "Original failure".into(), lifetime });
            assert_eq!(locale::<NoChild>(failure.as_ref()).id, selected.id);
            let mut shown = false;
            let original = failure.as_ref() as *const dyn Error;
            let returned = crate::production::entry_failure::report_failure::<
                NoChild,
                std::io::Error,
            >(failure, |_, _, summary, details| {
                assert_eq!(summary, selected.report.startup_failed);
                assert!(details.to_string().contains("Original failure"));
                shown = true;
                Ok(())
            });
            assert!(shown);
            assert!(std::ptr::eq(original, returned.as_ref() as *const dyn Error));
        }
        Ok(())
    }
}
