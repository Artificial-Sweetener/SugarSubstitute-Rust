// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present a final startup failure through the shared report surface after the failed
//! launch's native lifetime has retired.
use crate::{
    DesktopError,
    report::surface::{ReportPresentation, StartupReport, show_report},
};
use std::fmt::{self, Write};
use sugarsubstitute_contracts::localization::Messages;

/// Largest startup report text, matching the report surface's bound.
const MAX_DETAILS_BYTES: usize = 256 * 1024;

/// Show an early production-entry failure on the main thread after native teardown.
///
/// `copy` selects the user's language; `summary` is the localized heading and `details` the
/// already redacted cause chain. Details are capped at 256 KiB on a character boundary with
/// NUL replaced. This starts no execution, storage or incident-report lifecycle, and must not
/// be called from inside another window's event callback.
/// # Errors
/// Preserves diagnostic formatting, native window and rendering failures.
pub fn show_startup_failure(
    copy: &'static Messages,
    title: &str,
    summary: &str,
    details: fmt::Arguments<'_>,
) -> Result<(), DesktopError> {
    let details = Details::format(MAX_DETAILS_BYTES, copy.report.omitted, details)?;
    show_report(
        ReportPresentation::startup_failure(copy, title, summary, details),
        &mut StartupReport,
    )
    .map_err(|cause| DesktopError::boxed("startup_failure_report", cause))
}

/// Collect a scalar-safe diagnostic prefix without an unbounded intermediate String.
struct Details {
    text: String,
    limit: usize,
    truncated: bool,
}
impl Details {
    /// Format directly into the bounded text, ending with the omission marker when truncated.
    fn format(
        limit: usize,
        omitted: &str,
        arguments: fmt::Arguments<'_>,
    ) -> Result<String, DesktopError> {
        let limit = limit.saturating_sub(omitted.len() + 2);
        let mut details = Self { text: String::new(), limit, truncated: false };
        details
            .text
            .try_reserve(limit.min(64 * 1024))
            .map_err(|cause| DesktopError::new("startup_failure_text", cause))?;
        fmt::write(&mut details, arguments)
            .map_err(|cause| DesktopError::new("startup_failure_format", cause))?;
        if details.truncated {
            details.text.push('\n');
            details.text.push_str(omitted);
        }
        details.text.push('\n');
        Ok(details.text)
    }
}
impl Write for Details {
    /// Append a complete scalar prefix and omit all later fragments after truncation.
    fn write_str(&mut self, value: &str) -> fmt::Result {
        for scalar in value.chars() {
            if self.truncated {
                break;
            }
            let scalar = match scalar {
                '\0' => '\u{fffd}',
                '\r' => continue,
                other => other,
            };
            if self.text.len() + scalar.len_utf8() > self.limit {
                self.truncated = true;
                break;
            }
            self.text.push(scalar);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keep diagnostics readable without embedded terminators or split characters.
    #[test]
    fn details_keep_unicode_replace_nul_and_bound_the_original_prefix() -> Result<(), DesktopError>
    {
        let short = Details::format(64, "[omitted]", format_args!("Startup failed\r\n😀\0日本"))?;
        assert_eq!(short, "Startup failed\n😀\u{fffd}日本\n");
        let bounded = Details::format(20, "[omitted]", format_args!("123456😀discarded"))?;
        assert_eq!(bounded, "123456\n[omitted]\n");
        assert!(bounded.len() <= 20);
        Ok(())
    }

    /// A broken formatter must remain observable rather than produce a misleading empty report.
    #[test]
    fn diagnostic_formatting_failure_remains_an_observable_original_cause() {
        struct Failed;
        impl fmt::Display for Failed {
            fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
                Err(fmt::Error)
            }
        }
        let failure = Details::format(8192, "[omitted]", format_args!("{Failed}"));
        assert!(failure.is_err_and(|cause| {
            std::error::Error::source(&cause).is_some_and(|cause| cause.is::<fmt::Error>())
        }));
    }
}
