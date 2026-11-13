// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Paint bundled caption glyph alpha with the production device-pixel phase.

use std::{error::Error, fmt};
use sugarsubstitute_ui::bitmap::Bitmap;
#[path = "caption_render.rs"]
mod raster;
pub(crate) use raster::CaptionCursor;

/// Preserve invalid device scales and packaged icon decoder failures.
#[derive(Debug)]
pub enum IconError {
    /// The requested device scale is outside the bounded splash contract.
    InvalidScale,
    /// A packaged PNG could not be decoded.
    Decode(png::DecodingError),
    /// A reviewed PNG format or bounded buffer allocation failed, preserving its cause.
    Preparation(Box<dyn Error + Send + Sync>),
}
impl fmt::Display for IconError {
    /// Describe the failing caption operation for host diagnostics.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScale => formatter.write_str("Invalid caption device scale"),
            Self::Decode(error) => write!(formatter, "Cannot decode caption glyph: {error}"),
            Self::Preparation(error) => write!(formatter, "Cannot prepare caption glyph: {error}"),
        }
    }
}
impl Error for IconError {
    /// Preserve the packaged decoder cause across the presentation boundary.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            Self::Preparation(error) => Some(error.as_ref()),
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
    decoded: Option<(usize, [Bitmap; 2])>,
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
    #[cfg(test)]
    pub(crate) fn draw(
        &mut self,
        target: &mut Bitmap,
        scale: f64,
        kind: CaptionIcon,
        tint: [u8; 3],
    ) -> Result<(), IconError> {
        let mut cursor = CaptionCursor::new(scale, kind, tint)?;
        while !cursor.step(self, target)? {}
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
