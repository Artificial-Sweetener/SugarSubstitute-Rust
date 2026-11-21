// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The desktop body wash: the translucent fill the page paints over the window's Mica behind its
//! workspace, which the native canvas paints behind its image so both read as one surface.

/// The body wash for the window theme, as 8-bit sRGB channels and 8-bit alpha.
#[must_use]
pub const fn desktop_body_wash(dark: bool) -> [u8; 4] {
    if dark { [32, 32, 32, 150] } else { [251, 251, 251, 188] }
}

#[cfg(test)]
mod tests {
    use super::desktop_body_wash;

    /// The page's CSS and the native canvas paint the same wash: both match the shared record.
    #[test]
    fn the_body_wash_matches_the_shared_record() -> Result<(), Box<dyn std::error::Error>> {
        let vectors: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../Sugar/integrations/contracts/desktop_wash_vectors.json"
        ))?;
        for (theme, dark) in [("dark", true), ("light", false)] {
            let expected: Vec<u8> = serde_json::from_value(vectors["body"][theme].clone())?;
            assert_eq!(desktop_body_wash(dark).to_vec(), expected, "{theme}");
        }
        Ok(())
    }
}
