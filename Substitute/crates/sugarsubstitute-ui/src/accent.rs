// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own qfluentwidgets-compatible mode-dependent accent variants and semantic control roles.
//! The configured RGB seed remains unchanged; every shade is derived independently from it.

use crate::widgets::ControlState;
use sugarsubstitute_contracts::appearance::AccentColor;
mod hsv;

/// An opaque sRGB seed, resolved by the composition owner before shared widget preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccentPalette([u8; 3]);

impl AccentPalette {
    /// Canonical product pink used when no system accent is available.
    pub const PRODUCT: Self = Self([233, 30, 99]);

    /// Retain the exact operating-system seed independently of the surface's mode.
    pub const fn system(accent: AccentColor) -> Self {
        Self(accent.rgb())
    }

    /// Use the reported accent, or the canonical product seed.
    pub const fn resolve(accent: Option<AccentColor>) -> Self {
        match accent {
            Some(accent) => Self::system(accent),
            None => Self::PRODUCT,
        }
    }

    /// Derive all seven qfluentwidgets shades for the resolved surface mode.
    pub fn variants(self, dark: bool) -> AccentVariants {
        let hsv = hsv::Hsv::from_rgb(self.0);
        let s = if dark { hsv.saturation * 0.84 } else { hsv.saturation };
        let v = if dark { 1.0 } else { hsv.value };
        let shade = |saturation, value| hsv.rgb(s * saturation, v * value);
        AccentVariants {
            dark,
            primary: shade(1.0, 1.0),
            dark1: shade(1.0, if dark { 0.9 } else { 0.75 }),
            dark2: shade(if dark { 0.977 } else { 1.05 }, if dark { 0.82 } else { 0.5 }),
            dark3: shade(if dark { 0.95 } else { 1.1 }, if dark { 0.7 } else { 0.4 }),
            light1: shade(if dark { 0.92 } else { 1.0 }, if dark { 1.0 } else { 1.05 }),
            light2: shade(if dark { 0.78 } else { 0.75 }, if dark { 1.0 } else { 1.05 }),
            light3: shade(0.65, if dark { 1.0 } else { 1.05 }),
        }
    }

    /// Opaque progress completion uses the resolved mode's primary shade.
    pub fn progress(self, dark: bool) -> [u8; 4] {
        self.variants(dark).primary
    }
}

/// Independently derived qfluentwidgets shades, in unpremultiplied opaque RGBA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccentVariants {
    dark: bool,
    /// Default accent fill, indicator, progress, slider and focus shade.
    pub primary: [u8; 4],
    /// First darker shade; dark-mode button hover and pressed link.
    pub dark1: [u8; 4],
    /// Second darker shade; dark-mode button and checkbox press.
    pub dark2: [u8; 4],
    /// Third darker shade; available for accent separators.
    pub dark3: [u8; 4],
    /// First lighter shade; light-mode hover and switch hover in both modes.
    pub light1: [u8; 4],
    /// Second lighter shade; light-mode checkbox press and switch press in both modes.
    pub light2: [u8; 4],
    /// Third lighter shade; light-mode button press.
    pub light3: [u8; 4],
}

impl AccentVariants {
    /// Enabled primary button fill; disabled neutral treatment belongs to the widget.
    pub fn button(self, state: ControlState) -> [u8; 4] {
        match state {
            ControlState::Hover => {
                if self.dark {
                    self.dark1
                } else {
                    self.light1
                }
            }
            ControlState::Pressed => {
                if self.dark {
                    self.dark2
                } else {
                    self.light3
                }
            }
            _ => self.primary,
        }
    }

    /// Enabled checked checkbox fill and border, including its distinct light-mode press.
    pub fn checkbox(self, state: ControlState) -> [u8; 4] {
        if state == ControlState::Pressed && !self.dark { self.light2 } else { self.button(state) }
    }

    /// Enabled checked switch fill; both modes use lighter hover and pressed variants.
    pub fn toggle(self, state: ControlState) -> [u8; 4] {
        match state {
            ControlState::Hover => self.light1,
            ControlState::Pressed => self.light2,
            _ => self.primary,
        }
    }

    /// Enabled link foreground; only a dark-mode press changes its shade.
    pub fn link(self, state: ControlState) -> [u8; 4] {
        if self.dark && state == ControlState::Pressed { self.dark1 } else { self.primary }
    }

    /// Text-selection fill for the resolved mode.
    pub fn selection(self) -> [u8; 4] {
        if self.dark { self.primary } else { self.light1 }
    }

    /// Enabled text or icon on accent; pressed button text uses 63% opacity.
    pub fn foreground(self, state: ControlState) -> [u8; 4] {
        foreground(self.dark, state)
    }

    /// Apply a surface-owned wash opacity to the resolved primary shade.
    pub fn tint(self, alpha: u8) -> [u8; 4] {
        let [r, g, b, _] = self.primary;
        [r, g, b, alpha]
    }
}

/// Resolve seed-independent on-accent text for the actual surface mode and pointer state.
pub(crate) fn foreground(dark: bool, state: ControlState) -> [u8; 4] {
    let channel = if dark { 0 } else { 255 };
    [channel, channel, channel, if state == ControlState::Pressed { 161 } else { 255 }]
}

#[cfg(test)]
mod tests;
