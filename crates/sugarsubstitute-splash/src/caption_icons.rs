// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Paint bundled caption glyph alpha with the production device-pixel phase.

use std::{error::Error, fmt};
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

/// Preserve invalid device scales and packaged icon decoder failures.
#[derive(Debug)]
pub enum IconError {
    /// The requested device scale is outside the bounded splash contract.
    InvalidScale,
    /// A packaged PNG could not be decoded.
    Decode(png::DecodingError),
}
impl fmt::Display for IconError {
    /// Describe the failing caption operation for host diagnostics.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScale => formatter.write_str("Invalid caption device scale"),
            Self::Decode(error) => write!(formatter, "Cannot decode caption glyph: {error}"),
        }
    }
}
impl Error for IconError {
    /// Preserve the packaged decoder cause across the presentation boundary.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            Self::InvalidScale => None,
        }
    }
}

/// Identify the two glyphs while chrome retains button background/state ownership.
#[derive(Clone, Copy)]
pub(crate) enum CaptionIcon {
    Console,
    Close,
}

/// Retain only the selected resource pair as process-lifetime presentation data.
#[derive(Default)]
pub(crate) struct CaptionIcons {
    decoded: Option<(usize, [Pixmap; 2])>,
}
impl CaptionIcons {
    /// Defer PNG decoding until the first caption paint.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Tint glyph alpha and paint at the floor of its global logical button origin.
    ///
    /// # Errors
    /// Reject invalid scales and preserve failures decoding the selected resource pair.
    pub(crate) fn draw(
        &mut self,
        target: &mut Pixmap,
        scale: f64,
        kind: CaptionIcon,
        tint: [u8; 3],
    ) -> Result<(), IconError> {
        if !scale.is_finite() || !(0.5..=4.0).contains(&scale) {
            return Err(IconError::InvalidScale);
        }
        let index = ASSETS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (a.0 - scale).abs().total_cmp(&(b.0 - scale).abs()))
            .map_or(0, |(index, _)| index);
        if self.decoded.as_ref().is_none_or(|(loaded, _)| *loaded != index) {
            let (_, console, close) = ASSETS[index];
            self.decoded = Some((
                index,
                [
                    Pixmap::decode_png(console).map_err(IconError::Decode)?,
                    Pixmap::decode_png(close).map_err(IconError::Decode)?,
                ],
            ));
        }
        if let Some((_, icons)) = &self.decoded {
            let (position, offset) = match kind {
                CaptionIcon::Console => (0, 466.0),
                CaptionIcon::Close => (1, 512.0),
            };
            let mut glyph = icons[position].clone();
            for pixel in glyph.data_mut().chunks_exact_mut(4) {
                for channel in 0..3 {
                    let product = u16::from(tint[channel]) * u16::from(pixel[3]);
                    pixel[channel] = ((product + (product >> 8) + 128) >> 8) as u8;
                }
            }
            let ratio = (scale / ASSETS[index].0) as f32;
            let paint = PixmapPaint { quality: FilterQuality::Bilinear, ..PixmapPaint::default() };
            target.draw_pixmap(
                0,
                0,
                glyph.as_ref(),
                &paint,
                Transform::from_row(ratio, 0.0, 0.0, ratio, (offset * scale).floor() as f32, 0.0),
                None,
            );
        }
        Ok(())
    }
}

/// Package fixed white-alpha glyphs captured from the original MIT icon resources.
macro_rules! resource {
    ($scale:literal, $number:literal) => {
        (
            $scale,
            include_bytes!(concat!("../assets/caption/console-", $number, ".png")) as &[u8],
            include_bytes!(concat!("../assets/caption/close-", $number, ".png")) as &[u8],
        )
    };
}
const ASSETS: [(f64, &[u8], &[u8]); 9] = [
    resource!(1.0, "100"),
    resource!(1.25, "125"),
    resource!(1.5, "150"),
    resource!(1.75, "175"),
    resource!(2.0, "200"),
    resource!(2.5, "250"),
    resource!(3.0, "300"),
    resource!(3.5, "350"),
    resource!(4.0, "400"),
];

#[cfg(test)]
#[path = "caption_icons_tests.rs"]
mod tests;
