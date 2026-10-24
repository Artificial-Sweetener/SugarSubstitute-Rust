// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Decode packaged mascot poses on demand and paint the source splash's card geometry on CPU.

use crate::animation::{AnimationFrame, POSE_COUNT};
use crate::projection::Bounds;
use std::error::Error;
use std::fmt::{Display, Formatter};
use tiny_skia::Pixmap;

#[path = "character_decode.rs"]
mod decode;
#[path = "character_render.rs"]
mod raster;
#[path = "character_sampling.rs"]
mod sampling;
pub(crate) use raster::CharacterRender;

const POSE_SIZE: u32 = 386;
const GUEST_NAMES: [&str; 12] = [
    "cass.png",
    "comfy.png",
    "comfy2.png",
    "cubby.png",
    "diffusion.png",
    "liz.png",
    "lvgf.png",
    "married.png",
    "pixelandlibrary.png",
    "ren.png",
    "syl.png",
    "witchy.png",
];

/// Borrow one compiled-in PNG without filesystem discovery or startup-wide decoding.
#[derive(Clone, Copy, Debug)]
pub struct PoseAsset {
    /// Original PNG filename, ordered numerically before named guest poses.
    pub name: &'static str,
    /// Encoded original PNG bytes with process lifetime.
    pub bytes: &'static [u8],
}

/// Report an invalid packaged library or failed paint input at the character owner.
#[derive(Debug)]
pub enum CharacterError {
    /// The packaged table has a different pose count from the source application.
    AssetCount {
        /// Number of supplied assets.
        actual: usize,
    },
    /// A filename disagrees with the selector's source-compatible ordering.
    AssetOrder {
        /// Incorrect table position.
        index: usize,
        /// Supplied filename.
        name: &'static str,
    },
    /// Decoding a packaged PNG failed.
    Decode {
        /// Filename whose bytes failed validation.
        name: &'static str,
        /// Decoder error preserved for diagnostic chaining.
        source: Box<dyn Error + Send + Sync>,
    },
    /// The PNG has different dimensions from the original pose library.
    AssetSize {
        /// Filename whose image dimensions differ.
        name: &'static str,
        /// Decoded image width.
        width: u32,
        /// Decoded image height.
        height: u32,
    },
    /// The source asset is not the reviewed noninterlaced, single-frame RGBA8 representation.
    AssetFormat {
        /// Filename whose pixel representation differs.
        name: &'static str,
    },
    /// The supplied frame refers to a pose outside the packaged library.
    PoseIndex {
        /// Requested image index.
        index: usize,
    },
    /// The destination rectangle cannot be rendered safely.
    InvalidBounds,
    /// Allocating the bounded card coverage raster failed.
    RasterAllocation,
    /// Preserve the cause of failed transformed-card coverage allocation.
    Coverage {
        /// Original bounded coverage owner failure.
        source: Box<dyn Error + Send + Sync>,
    },
}

impl Display for CharacterError {
    /// Describe failures without disclosing installation-specific paths.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AssetCount { actual } => {
                write!(formatter, "Expected {POSE_COUNT} splash poses, received {actual}")
            }
            Self::AssetOrder { index, name } => {
                write!(formatter, "Splash pose {name} has invalid ordering at index {index}")
            }
            Self::Decode { name, source } => {
                write!(formatter, "Cannot decode splash pose {name}: {source}")
            }
            Self::AssetSize { name, width, height } => write!(
                formatter,
                "Splash pose {name} is {width}×{height}; expected {POSE_SIZE}×{POSE_SIZE}"
            ),
            Self::AssetFormat { name } => {
                write!(
                    formatter,
                    "Splash pose {name} must contain noninterlaced, single-frame RGBA8 pixels"
                )
            }
            Self::PoseIndex { index } => {
                write!(formatter, "Splash pose index {index} is out of range")
            }
            Self::InvalidBounds => {
                formatter.write_str("Splash card bounds must be positive and finite")
            }
            Self::RasterAllocation => {
                formatter.write_str("Cannot allocate splash card coverage raster")
            }
            Self::Coverage { source } => {
                write!(formatter, "Cannot allocate splash card coverage: {source}")
            }
        }
    }
}

impl Error for CharacterError {
    /// Retain decoder and coverage-allocation causes for structured runtime diagnostics.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode { source, .. } | Self::Coverage { source } => Some(source.as_ref()),
            _ => None,
        }
    }
}

/// Own decoded, premultiplied PNGs for one splash's process-lifetime presentation.
///
/// Decode only poses requested by `render`. This keeps the bootstrap character independent
/// of the other 39 PNGs. Dropping the library releases every decoded raster; it writes no
/// persistent cache and has no application or canvas state.
pub struct CharacterLibrary {
    assets: &'static [PoseAsset],
    decoded: Vec<Option<Pixmap>>,
}

impl CharacterLibrary {
    /// Validate table metadata without decoding PNGs or loading filesystem resources.
    ///
    /// # Errors
    /// Return an asset-count or ordering error when the source library contract differs.
    pub fn new(assets: &'static [PoseAsset]) -> Result<Self, CharacterError> {
        if assets.len() != POSE_COUNT {
            return Err(CharacterError::AssetCount { actual: assets.len() });
        }
        for (index, asset) in assets.iter().enumerate() {
            let ordered = if index < 28 {
                asset.name.strip_suffix(".png").and_then(|stem| stem.parse::<usize>().ok())
                    == Some(index + 1)
            } else {
                GUEST_NAMES[index - 28] == asset.name
            };
            if !ordered {
                return Err(CharacterError::AssetOrder { index, name: asset.name });
            }
        }
        let mut decoded = Vec::new();
        decoded
            .try_reserve_exact(assets.len())
            .map_err(|source| CharacterError::Coverage { source: Box::new(source) })?;
        decoded.resize_with(assets.len(), || None);
        Ok(Self { assets, decoded })
    }

    /// Paint one pose over the existing target using the source's aspect-preserving card.
    ///
    /// Bounds and target pixels share a coordinate system; the caller applies display scale
    /// before calling. The source rectangle spans zero through the PNG extent, exactly as
    /// QPainter's `drawPixmap` overload, without a half-pixel translation. Source-over paint
    /// preserves the caller's backdrop. Paint is clipped to the mascot widget bounds and
    /// the target's edges, including airborne card corners.
    ///
    /// # Errors
    /// Return a geometry, pose-index, PNG-decoding, packaged-image-size, or allocation error.
    pub fn render(
        &mut self,
        target: &mut crate::Raster,
        bounds: Bounds,
        frame: AnimationFrame,
    ) -> Result<(), CharacterError> {
        let mut cursor = CharacterRender::new(target.width(), target.height(), bounds, frame)?;
        while !cursor.step(self, &mut target.pixels)? {}
        Ok(())
    }

    /// Report retained decoded images for startup and process-lifetime cache qualification.
    pub fn decoded_count(&self) -> usize {
        self.decoded.iter().filter(|pose| pose.is_some()).count()
    }
}

#[cfg(test)]
#[path = "character_tests.rs"]
mod tests;
