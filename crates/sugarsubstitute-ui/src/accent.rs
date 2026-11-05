// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve the accent roles native widgets paint from the operating-system accent.
//!
//! Surfaces receive one palette from their composition owner and pass it to every shared
//! renderer. When the platform reports no accent, the product palette is the defined fallback.

use sugarsubstitute_contracts::appearance::AccentColor;

/// Accent colors for every shared widget role, as unpremultiplied opaque RGBA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccentPalette {
    progress_dark: [u8; 3],
    progress_light: [u8; 3],
    fill: [u8; 3],
    hover: [u8; 3],
}

impl AccentPalette {
    /// Product pink used when the operating system reports no accent. Its theme-specific
    /// progress shades and hover shade retain the original Fluent reference values.
    pub const PRODUCT: Self = Self {
        progress_dark: [255, 68, 132],
        progress_light: [233, 30, 99],
        fill: [255, 64, 137],
        hover: [243, 58, 128],
    };

    /// Paint every accent role with the operating-system accent; hover darkens it slightly.
    pub const fn system(accent: AccentColor) -> Self {
        let rgb = accent.rgb();
        Self { progress_dark: rgb, progress_light: rgb, fill: rgb, hover: darken(rgb) }
    }

    /// Use the reported accent, or the product palette when the platform reports none.
    pub const fn resolve(accent: Option<AccentColor>) -> Self {
        match accent {
            Some(accent) => Self::system(accent),
            None => Self::PRODUCT,
        }
    }

    /// Opaque progress completion color for a dark or light background.
    pub const fn progress(self, dark: bool) -> [u8; 4] {
        opaque(if dark { self.progress_dark } else { self.progress_light })
    }

    /// Opaque emphasized control fill, such as a primary button or a checked box.
    pub const fn fill(self) -> [u8; 4] {
        opaque(self.fill)
    }

    /// Emphasized control fill under the pointer or during a press.
    pub const fn hover(self) -> [u8; 4] {
        opaque(self.hover)
    }

    /// Translucent emphasized fill, such as an icon badge wash.
    pub const fn tint(self, alpha: u8) -> [u8; 4] {
        let [red, green, blue] = self.fill;
        [red, green, blue, alpha]
    }
}

const fn opaque([red, green, blue]: [u8; 3]) -> [u8; 4] {
    [red, green, blue, 255]
}

/// Scale each channel to 95%, the product hover's approximate darkening, rounding to nearest.
const fn darken([red, green, blue]: [u8; 3]) -> [u8; 3] {
    const fn channel(value: u8) -> u8 {
        ((value as u16 * 95 + 50) / 100) as u8
    }
    [channel(red), channel(green), channel(blue)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Without a reported accent, native surfaces keep the original product colors exactly.
    #[test]
    fn missing_system_accent_falls_back_to_the_product_palette() {
        let palette = AccentPalette::resolve(None);
        assert_eq!(palette, AccentPalette::PRODUCT);
        assert_eq!(palette.progress(true), [255, 68, 132, 255]);
        assert_eq!(palette.progress(false), [233, 30, 99, 255]);
        assert_eq!(palette.fill(), [255, 64, 137, 255]);
        assert_eq!(palette.hover(), [243, 58, 128, 255]);
        assert_eq!(palette.tint(31), [255, 64, 137, 31]);
    }

    /// A reported accent replaces every role, independent of theme.
    #[test]
    fn system_accent_supplies_every_role() {
        let palette = AccentPalette::resolve(Some(AccentColor::new(0, 120, 212)));
        assert_eq!(palette.progress(true), [0, 120, 212, 255]);
        assert_eq!(palette.progress(false), [0, 120, 212, 255]);
        assert_eq!(palette.fill(), [0, 120, 212, 255]);
        assert_eq!(palette.hover(), [0, 114, 201, 255]);
        assert_eq!(palette.tint(61), [0, 120, 212, 61]);
    }
}
