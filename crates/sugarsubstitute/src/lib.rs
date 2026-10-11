// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Establish SugarSubstitute's product identity for the Rust desktop host.

/// Keep the product name independent of the source checkout directory.
pub const APPLICATION_NAME: &str = "SugarSubstitute";

#[cfg(test)]
mod tests {
    /// Preserve the application identity expected by existing installations.
    #[test]
    fn product_name_preserves_existing_identity() {
        assert_eq!(super::APPLICATION_NAME, "SugarSubstitute");
    }
}
