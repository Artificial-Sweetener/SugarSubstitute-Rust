// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Portable package names avoid Windows aliases even when decoded on another platform.
use super::Rejection;

/// Validate a relative slash-separated distribution name without filesystem normalization.
/// # Errors
/// Rejects traversal, alternate streams, device names, separators and ambiguous components.
pub fn relative_path(value: &str) -> Result<(), Rejection> {
    if value.is_empty() || value.len() > 1024 || value.contains([':', '\\', '\0']) {
        return Err(Rejection::Path);
    }
    for part in value.split('/') {
        if part.is_empty()
            || matches!(part, "." | "..")
            || part.ends_with(['.', ' '])
            || part.encode_utf16().count() > 255
            || part.chars().any(|ch| ch.is_control() || "<>\"|?*".contains(ch))
        {
            return Err(Rejection::Path);
        }
        let stem =
            part.split('.').next().unwrap_or_default().trim_end_matches(' ').to_ascii_uppercase();
        if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
            || ["COM", "LPT"].iter().any(|prefix| {
                stem.strip_prefix(prefix).is_some_and(|suffix| {
                    matches!(
                        suffix,
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    )
                })
            })
        {
            return Err(Rejection::Path);
        }
    }
    Ok(())
}
