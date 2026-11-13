// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read and render the Comfy Registry `.tracking` ownership record.
//!
//! ComfyUI Manager writes the archive's member names joined by newlines in text mode, so the
//! record uses the platform line separator with no trailing separator. Readers accept LF or
//! CRLF, strip each line as Manager does and skip blank lines. The record names exactly the
//! paths the registry install owns; everything else in the folder is user data.

use super::entry::{EntryName, parse_entry, validate_entry_set};
use super::error::{ErrorCode, PackInstallError, Result};

/// The layout version the shared install-format catalog records for `.tracking`.
///
/// The record itself carries no version, because ComfyUI Manager defines it.
pub const TRACKING_FORMAT_VERSION: u32 = 1;
/// The largest record read.
pub const MAX_TRACKING_BYTES: usize = 4 * 1024 * 1024;
/// The most entries one record may list.
pub const MAX_TRACKED_ENTRIES: usize = 20_000;

/// Return the validated entries of one record, in recorded order.
///
/// # Errors
/// Returns `tracking-too-large`, `tracking-unreadable` for non-UTF-8 bytes, any entry
/// refusal, `too-many-entries`, or a set refusal.
pub fn parse_tracking(content: &[u8]) -> Result<Vec<EntryName>> {
    if content.len() > MAX_TRACKING_BYTES {
        return Err(PackInstallError::format(
            ErrorCode::TrackingTooLarge,
            "The ownership record exceeds the size limit.",
        ));
    }
    let text = std::str::from_utf8(content).map_err(|_| {
        PackInstallError::format(
            ErrorCode::TrackingUnreadable,
            "The ownership record is not UTF-8 text.",
        )
    })?;
    let entries = text
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_entry)
        .collect::<Result<Vec<_>>>()?;
    if entries.len() > MAX_TRACKED_ENTRIES {
        return Err(PackInstallError::format(
            ErrorCode::TooManyEntries,
            "The ownership record lists too many entries.",
        ));
    }
    validate_entry_set(&entries)?;
    Ok(entries)
}

/// Return the logical record text with LF separators.
pub fn render_tracking(entries: &[EntryName]) -> String {
    entries.iter().map(EntryName::text).collect::<Vec<_>>().join("\n")
}

/// Return the exact bytes ComfyUI Manager writes on this platform.
pub fn tracking_bytes(entries: &[EntryName]) -> Vec<u8> {
    let separator = if cfg!(windows) { "\r\n" } else { "\n" };
    entries.iter().map(EntryName::text).collect::<Vec<_>>().join(separator).into_bytes()
}
