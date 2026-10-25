// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Preserve actionable input feedback independently of its original diagnostic cause.
use std::{error::Error, fmt, io};
use sugarsubstitute_contracts::localization::installer;

enum Kind {
    AbsoluteFolder,
    Choices,
}
/// Typed user input rejection retains the original validation error for diagnostics.
pub(super) struct InputFailure {
    kind: Kind,
    cause: Box<dyn Error>,
}
impl InputFailure {
    /// Select an actionable folder message without classifying by error-string spelling.
    pub(super) fn absolute() -> Self {
        Self { kind: Kind::AbsoluteFolder, cause: "Choose an absolute installation folder".into() }
    }
    /// Retain the exact native selection validation error as the diagnostic source.
    pub(super) fn selection(cause: io::Error) -> Self {
        Self { kind: Kind::Choices, cause: Box::new(cause) }
    }
}
impl fmt::Debug for InputFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InstallerInputFailure")
    }
}
impl fmt::Display for InputFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Installer input validation failed")
    }
}
impl Error for InputFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
/// Unexpected/internal failures keep generic localized recovery; known input failures retain guidance.
pub(super) fn message(cause: &(dyn Error + 'static), locale: &str) -> &'static str {
    let copy = installer::messages(locale);
    match cause.downcast_ref::<InputFailure>().map(|failure| &failure.kind) {
        Some(Kind::AbsoluteFolder) => copy.choose_absolute_folder,
        Some(Kind::Choices) => copy.review_choices,
        None => copy.setup_failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actionable_input_feedback_keeps_all_current_locales_and_original_io_causes() {
        let selection = InputFailure::selection(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Original folder validation cause",
        ));
        for locale in sugarsubstitute_contracts::localization::LOCALES {
            assert_eq!(message(&selection, locale.id), locale.installer.review_choices);
            assert_eq!(
                message(&InputFailure::absolute(), locale.id),
                locale.installer.choose_absolute_folder
            );
            assert_eq!(
                message(&io::Error::other("Internal cause"), locale.id),
                locale.installer.setup_failed
            );
        }
        let original = selection.source().and_then(|cause| cause.downcast_ref::<io::Error>());
        assert!(original.is_some_and(|cause| cause.kind() == io::ErrorKind::InvalidInput
            && cause.to_string() == "Original folder validation cause"));
    }
}
