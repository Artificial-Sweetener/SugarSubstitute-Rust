// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present bounded final Root failure details after its native lifetime has retired.
use crate::DesktopError;
use std::fmt::{self, Write};

/// Display an early production-entry failure on the main UI thread after native teardown.
///
/// The caller supplies localized product copy and already redacted diagnostic details,
/// retaining the original failure until dismissal. Internal captured roles must use
/// their authenticated error transport rather than invoke this modal surface.
/// Details are capped at 8,192 UTF-16 units, preserving scalar boundaries and replacing
/// embedded NULs. This call starts no execution, storage or incident-report lifecycle.
/// # Errors
/// Preserves allocation, diagnostic formatting and native dialog failures.
#[cfg(target_os = "windows")]
pub fn show_startup_failure(
    title: &str,
    summary: &str,
    details: fmt::Arguments<'_>,
) -> Result<(), DesktopError> {
    use windows::{
        Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
        core::PCWSTR,
    };
    let title = Text::format(256, format_args!("{title}"))?;
    let message = Text::format(8192, format_args!("{summary}\n\n{details}"))?;
    // SAFETY: Both bounded NUL-terminated buffers remain owned through the synchronous
    // modal call. The caller's previous native window and its callbacks have retired.
    let result = unsafe {
        MessageBoxW(None, PCWSTR(message.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONERROR)
    };
    if result.0 == 0 {
        return Err(DesktopError::new(
            "startup_failure_dialog",
            windows::core::Error::from_thread(),
        ));
    }
    Ok(())
}

/// Collect one scalar-safe diagnostic prefix without an unbounded intermediate String.
struct Text {
    units: Vec<u16>,
    limit: usize,
    truncated: bool,
}
impl Text {
    /// Format directly into the bounded UTF-16 buffer, reserving truncation and termination space.
    fn format(limit: usize, arguments: fmt::Arguments<'_>) -> Result<Vec<u16>, DesktopError> {
        let mut text = Self { units: Vec::new(), limit, truncated: false };
        text.units
            .try_reserve_exact(limit + 1)
            .map_err(|cause| DesktopError::new("startup_failure_text", cause))?;
        fmt::write(&mut text, arguments)
            .map_err(|cause| DesktopError::new("startup_failure_format", cause))?;
        if text.truncated {
            text.units.push('…' as u16);
        }
        text.units.push(0);
        Ok(text.units)
    }
}
impl Write for Text {
    /// Append a complete scalar prefix and omit all later fragments after truncation.
    fn write_str(&mut self, value: &str) -> fmt::Result {
        for scalar in value.chars() {
            if self.truncated {
                break;
            }
            let scalar = if scalar == '\0' { '\u{fffd}' } else { scalar };
            if self.units.len() + scalar.len_utf16() >= self.limit {
                self.truncated = true;
                break;
            }
            self.units.extend_from_slice(scalar.encode_utf16(&mut [0; 2]));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keep diagnostics readable without embedded terminators or split surrogate pairs.
    #[test]
    fn final_failure_details_keep_unicode_replace_nul_and_bound_the_original_prefix()
    -> Result<(), DesktopError> {
        let short = Text::format(32, format_args!("Startup failed\n😀\0日本"))?;
        assert_eq!(
            String::from_utf16_lossy(&short[..short.len() - 1]),
            "Startup failed\n😀\u{fffd}日本"
        );
        let bounded = Text::format(8, format_args!("123456😀discarded"))?;
        assert_eq!(bounded, "123456…\0".encode_utf16().collect::<Vec<_>>());
        assert!(String::from_utf16(&bounded[..bounded.len() - 1]).is_ok());
        assert!(bounded.len() <= 9);
        Ok(())
    }

    /// A broken formatter must remain observable rather than produce a misleading empty dialog.
    #[test]
    fn diagnostic_formatting_failure_remains_an_observable_original_cause() {
        struct Failed;
        impl fmt::Display for Failed {
            fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
                Err(fmt::Error)
            }
        }
        let failure = Text::format(8192, format_args!("{Failed}"));
        assert!(failure.is_err_and(|cause| {
            std::error::Error::source(&cause).is_some_and(|cause| cause.is::<fmt::Error>())
        }));
    }
}
