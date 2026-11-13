// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Emphasize one mask overlay, as a hovered region row does, outside authored content.
use super::*;

/// The `QColor::lighter` factor the input canvas uses for hovered overlays.
const HIGHLIGHT_FACTOR: u32 = 135;
const CHANNEL_MAX: u32 = u16::MAX as u32;
/// Qt's marker for an undefined (achromatic) hue.
const ACHROMATIC: u32 = u16::MAX as u32;

impl MaskAppearance {
    /// This appearance drawn emphasized: its colour lightened exactly as Qt's
    /// `QColor::lighter(135)` lightens it, at the same opacity.
    pub fn highlighted(self) -> Self {
        Self { color: lighter(self.color, HIGHLIGHT_FACTOR), ..self }
    }
}

impl Document {
    /// Emphasize one mask layer's overlay, or none. Emphasis publishes a new revision so frames
    /// redraw, but never a history step; undo and redo keep it while its layer remains.
    /// # Errors
    /// Rejects a layer that is absent or not a mask, and exhausted revision identity.
    pub fn set_highlight(&mut self, layer: Option<LayerId>) -> Result<bool, CanvasError> {
        if layer == self.current.highlight {
            return Ok(false);
        }
        if let Some(id) = layer
            && !self.current.content.layers.iter().any(|candidate| {
                candidate.id == id && matches!(candidate.content, LayerContent::Coverage { .. })
            })
        {
            return Err(CanvasError::Invalid("Only a present mask layer can be emphasized"));
        }
        let revision = self.next_revision()?;
        self.current = DocumentSnapshot { revision, highlight: layer, ..self.current.clone() };
        Ok(true)
    }
}

/// Qt's `QColor::lighter`: scale the HSV value by `factor` percent in 16-bit channels, taking
/// any overflow out of the saturation, then return to 8-bit sRGB.
fn lighter(color: [u8; 3], factor: u32) -> [u8; 3] {
    let wide = color.map(|channel| u32::from(channel) * 0x101);
    let (hue, saturation, value) = hsv(wide);
    let mut saturation = i64::from(saturation);
    let mut value = factor * value / 100;
    if value > CHANNEL_MAX {
        saturation = (saturation - i64::from(value - CHANNEL_MAX)).max(0);
        value = CHANNEL_MAX;
    }
    rgb(hue, saturation as u32, value).map(|channel| {
        let channel = channel as i32;
        ((channel - (channel >> 8) + 0x80) >> 8) as u8
    })
}

/// Round as Qt's `qRound` does for the nonnegative values used here.
fn round(value: f32) -> u32 {
    (value + 0.5) as u32
}

/// Qt's 16-bit HSV: hue in hundredths of a degree, or [`ACHROMATIC`], saturation and value.
fn hsv([red, green, blue]: [u32; 3]) -> (u32, u32, u32) {
    let [red, green, blue] = [red, green, blue].map(|channel| channel as f32 / CHANNEL_MAX as f32);
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let delta = max - min;
    let value = round(max * CHANNEL_MAX as f32);
    if delta.abs() <= 0.000_01 {
        return (ACHROMATIC, 0, value);
    }
    let saturation = round(delta / max * CHANNEL_MAX as f32);
    let mut hue = if red == max {
        (green - blue) / delta
    } else if green == max {
        2.0 + (blue - red) / delta
    } else {
        4.0 + (red - green) / delta
    } * 60.0;
    if hue < 0.0 {
        hue += 360.0;
    }
    (round(hue * 100.0), saturation, value)
}

/// Qt's HSV to 16-bit RGB conversion.
fn rgb(hue: u32, saturation: u32, value: u32) -> [u32; 3] {
    if saturation == 0 || hue == ACHROMATIC {
        return [value; 3];
    }
    let sector = if hue == 36_000 { 0.0 } else { hue as f32 / 6000.0 };
    let saturation = saturation as f32 / CHANNEL_MAX as f32;
    let value = value as f32 / CHANNEL_MAX as f32;
    let index = sector as u32;
    let fraction = sector - index as f32;
    let low = value * (1.0 - saturation);
    let falling = value * (1.0 - saturation * fraction);
    let rising = value * (1.0 - saturation * (1.0 - fraction));
    let channels = match index {
        0 => [value, rising, low],
        1 => [falling, value, low],
        2 => [low, value, rising],
        3 => [low, falling, value],
        4 => [rising, low, value],
        _ => [value, low, falling],
    };
    channels.map(|channel| round(channel * CHANNEL_MAX as f32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lightening_matches_qt_for_saturated_dark_and_gray_colours() {
        // Value overflow is taken out of saturation, as Qt does for bright colours.
        assert_eq!(lighter([255, 0, 0], 135), [255, 89, 89]);
        assert_eq!(lighter([100, 50, 0], 135), [135, 68, 0]);
        assert_eq!(lighter([0, 120, 215], 135), [35, 158, 255]);
        assert_eq!(lighter([128, 128, 128], 135), [173, 173, 173]);
        assert_eq!(lighter([255, 255, 255], 135), [255, 255, 255]);
        assert_eq!(lighter([0, 0, 0], 135), [0, 0, 0]);
    }
}
