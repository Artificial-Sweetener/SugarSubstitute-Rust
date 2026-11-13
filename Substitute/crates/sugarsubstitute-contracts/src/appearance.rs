// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Represent operating-system appearance facts independently of the platform that reports them.

use std::fmt;

/// An opaque sRGB accent color reported by the operating system.
///
/// The desktop adapter reads the platform accent; native surfaces derive their palettes from
/// it and the SugarCubes panel presentation carries it as [`Self::css_hex`]. Absence of a
/// reported accent is modelled by callers as `Option<AccentColor>`, never by a sentinel color.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AccentColor {
    red: u8,
    green: u8,
    blue: u8,
}

impl AccentColor {
    /// Retain exact 8-bit sRGB channels; platform alpha is not part of an accent.
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    /// Return the red, green and blue channels in that order.
    pub const fn rgb(self) -> [u8; 3] {
        [self.red, self.green, self.blue]
    }

    /// Format the color as the lowercase `#rrggbb` spelling accepted by the panel schema.
    pub fn css_hex(self) -> String {
        self.to_string()
    }
}

impl fmt::Display for AccentColor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }
}

#[cfg(test)]
mod tests {
    use super::AccentColor;

    /// The panel schema accepts only six hexadecimal digits after a single number sign.
    #[test]
    fn css_hex_is_exact_lowercase_six_digit_rgb() {
        assert_eq!(AccentColor::new(0, 120, 212).css_hex(), "#0078d4");
        assert_eq!(AccentColor::new(255, 64, 137).css_hex(), "#ff4089");
        assert_eq!(AccentColor::new(1, 2, 3).rgb(), [1, 2, 3]);
    }
}
