// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Paint the caption glyphs from the shared SVG icons at the device scale of each frame.

use std::{error::Error, fmt};
#[cfg(test)]
use sugarsubstitute_ui::bitmap::Bitmap;
use sugarsubstitute_ui::{shared_icons::SharedIcon, vector::VectorAsset};
#[path = "caption_render.rs"]
mod raster;
pub(crate) use raster::CaptionCursor;

/// Preserve invalid device scales and glyph preparation failures.
#[derive(Debug)]
pub enum IconError {
    /// The requested device scale is outside the bounded splash contract.
    InvalidScale,
    /// A shared icon could not be parsed or rasterized, preserving its cause.
    Preparation(Box<dyn Error + Send + Sync>),
}
impl fmt::Display for IconError {
    /// Describe the failing caption operation for host diagnostics.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScale => formatter.write_str("Invalid caption device scale"),
            Self::Preparation(error) => write!(formatter, "Cannot prepare caption glyph: {error}"),
        }
    }
}
impl Error for IconError {
    /// Preserve the preparation cause across the presentation boundary.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Preparation(error) => Some(error.as_ref()),
            Self::InvalidScale => None,
        }
    }
}

/// The splash's two caption glyphs while chrome retains button background and state ownership.
/// The console glyph shows where the toggle goes: the solid bar while the console is closed,
/// the hollow bar while it is open.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CaptionIcon {
    Console { open: bool },
    Close,
}

impl CaptionIcon {
    /// The shared artwork this glyph draws.
    pub(crate) const fn shared(self) -> SharedIcon {
        match self {
            Self::Console { open } => SharedIcon::console_toggle(open),
            Self::Close => SharedIcon::WindowClose,
        }
    }

    /// The glyph's box in logical splash pixels: the console glyph is 16 pixels centred in its
    /// 46 by 32 button at x 466; the close artwork spans its whole button at x 512.
    pub(crate) const fn bounds(self) -> [f32; 4] {
        match self {
            Self::Console { .. } => [466.0 + 15.0, 8.0, 16.0, 16.0],
            Self::Close => [512.0, 0.0, 46.0, 32.0],
        }
    }
}

/// Parsed shared artwork, kept for the process once a glyph first paints.
#[derive(Default)]
pub(crate) struct CaptionIcons {
    parsed: Vec<(SharedIcon, VectorAsset)>,
}
impl CaptionIcons {
    /// Defer parsing until the first caption paint.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Borrow one icon's parsed artwork, parsing it on first use.
    fn asset(&mut self, icon: SharedIcon) -> Result<&VectorAsset, IconError> {
        if let Some(index) = self.parsed.iter().position(|(kind, _)| *kind == icon) {
            return Ok(&self.parsed[index].1);
        }
        let asset = VectorAsset::parse(icon.source()).map_err(preparation)?;
        self.parsed.try_reserve(1).map_err(preparation)?;
        self.parsed.push((icon, asset));
        let last = self.parsed.len() - 1;
        Ok(&self.parsed[last].1)
    }

    /// Paint one glyph to completion.
    ///
    /// # Errors
    /// Reject invalid scales and preserve preparation failures.
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

/// Keep an allocator or parser cause without exposing its type publicly.
fn preparation(source: impl Error + Send + Sync + 'static) -> IconError {
    IconError::Preparation(Box::new(source))
}

#[cfg(test)]
#[path = "caption_icons_tests.rs"]
mod tests;
