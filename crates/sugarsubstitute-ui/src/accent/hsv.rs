// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match QColor's float HSV conversion and quantization at the RGB/HSV boundaries.
//! qfluentwidgets applies its scale factors in double precision between these boundaries.

/// Quantized hue and single-precision QColor components widened for Python's scalar maths.
pub(super) struct Hsv {
    hue: u16,
    pub(super) saturation: f64,
    pub(super) value: f64,
}

impl Hsv {
    /// Convert 8-bit sRGB to QColor's hundredth-degree hue and 16-bit saturation.
    pub(super) fn from_rgb(rgb: [u8; 3]) -> Self {
        let [r, g, b] = rgb.map(|c| f32::from(c) / 255.0);
        let maximum = r.max(g).max(b);
        let delta = maximum - r.min(g).min(b);
        let (hue, saturation) = if delta == 0.0 {
            (u16::MAX, 0.0)
        } else {
            let sector = if r == maximum {
                (g - b) / delta
            } else if g == maximum {
                2.0 + (b - r) / delta
            } else {
                4.0 + (r - g) / delta
            };
            let degrees = sector * 60.0;
            let degrees = if degrees < 0.0 { degrees + 360.0 } else { degrees };
            ((degrees * 100.0).round() as u16, ((delta / maximum) * 65535.0).round() / 65535.0)
        };
        Self { hue, saturation: f64::from(saturation), value: f64::from(maximum) }
    }

    /// Clamp derived components, quantize to 16 bits, then convert through float RGB to bytes.
    pub(super) fn rgb(&self, saturation: f64, value: f64) -> [u8; 4] {
        let quantize = |c: f64| ((c.min(1.0) as f32) * 65535.0).round() / 65535.0;
        let s = quantize(saturation);
        let v = quantize(value);
        let rgb = if self.hue == u16::MAX || s == 0.0 {
            [v; 3]
        } else {
            let h = if self.hue == 36000 { 0.0 } else { f32::from(self.hue) / 6000.0 };
            let sector = h as usize;
            let fraction = h - sector as f32;
            let p = v * (1.0 - s);
            let q = v * (1.0 - s * fraction);
            let t = v * (1.0 - s * (1.0 - fraction));
            match sector {
                0 => [v, t, p],
                1 => [q, v, p],
                2 => [p, v, t],
                3 => [p, q, v],
                4 => [t, p, v],
                _ => [v, p, q],
            }
        };
        let [r, g, b] = rgb.map(|c| (((c * 65535.0).round() as u32 + 128) / 257) as u8);
        [r, g, b, 255]
    }
}
