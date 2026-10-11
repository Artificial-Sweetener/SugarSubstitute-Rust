// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Decode packaged mascot poses on demand and paint the source splash's card geometry on CPU.

use crate::animation::{AnimationFrame, POSE_COUNT};
use crate::projection::{Bounds, Point, projected_quad};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::Cursor;
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

#[path = "character_sampling.rs"]
mod sampling;

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
    /// The source asset is not the reviewed eight-bit RGBA PNG representation.
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
                write!(formatter, "Splash pose {name} must contain eight-bit RGBA pixels")
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
        }
    }
}

impl Error for CharacterError {
    /// Keep the original decoder cause available to structured runtime diagnostics.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode { source, .. } => Some(source.as_ref()),
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
        Ok(Self { assets, decoded: std::iter::repeat_with(|| None).take(assets.len()).collect() })
    }

    /// Paint one pose over the existing target using the source's aspect-preserving card.
    ///
    /// Bounds and target pixels share a coordinate system; the caller applies display scale
    /// before calling. The source rectangle spans zero through the PNG extent, exactly as
    /// QPainter's `drawPixmap` overload, without a half-pixel translation. Source-over paint
    /// preserves the caller's backdrop and clipping at the target's edges.
    ///
    /// # Errors
    /// Return a geometry, pose-index, PNG-decoding, or packaged-image-size error.
    pub fn render(
        &mut self,
        target: &mut Pixmap,
        bounds: Bounds,
        frame: AnimationFrame,
    ) -> Result<(), CharacterError> {
        if !bounds.is_valid()
            || [bounds.x, bounds.y, bounds.width, bounds.height]
                .iter()
                .any(|value| value.abs() > f64::from(f32::MAX))
        {
            return Err(CharacterError::InvalidBounds);
        }
        let source = self.decode(frame.pose)?;
        let card_size = bounds.width.min(bounds.height);
        let card = Bounds {
            x: bounds.x + (bounds.width - card_size) / 2.0,
            y: bounds.y + (bounds.height - card_size) / 2.0,
            width: card_size,
            height: card_size,
        };
        let quad = projected_quad(card, frame.progress, frame.direction);
        let extent = f64::from(POSE_SIZE);
        let transform = card_transform(quad, extent);
        if transform.sx == 1.0 && transform.sy == 1.0 && transform.kx == 0.0 && transform.ky == 0.0
        {
            target.draw_pixmap(
                0,
                0,
                source.as_ref(),
                &PixmapPaint { quality: FilterQuality::Bilinear, ..PixmapPaint::default() },
                transform,
                None,
            );
        } else {
            sampling::paint(target, source, quad)?;
        }
        paint_paper_edge(target, quad, frame.progress, card_size / extent, bounds);
        Ok(())
    }

    /// Report retained decoded images for startup and process-lifetime cache qualification.
    pub fn decoded_count(&self) -> usize {
        self.decoded.iter().filter(|pose| pose.is_some()).count()
    }

    /// Validate and retain one requested PNG, preserving decoder error context.
    fn decode(&mut self, index: usize) -> Result<&Pixmap, CharacterError> {
        let asset = self.assets.get(index).ok_or(CharacterError::PoseIndex { index })?;
        let slot = self.decoded.get_mut(index).ok_or(CharacterError::PoseIndex { index })?;
        if slot.is_none() {
            *slot = Some(decode_asset(*asset)?);
        }
        slot.as_ref().ok_or(CharacterError::PoseIndex { index })
    }
}

/// Decode the reviewed PNG representation and retain the reference's byte-order arithmetic.
fn decode_asset(asset: PoseAsset) -> Result<Pixmap, CharacterError> {
    let mut decoder = png::Decoder::new(Cursor::new(asset.bytes));
    decoder.set_limits(png::Limits { bytes: 1024 * 1024 });
    let mut reader = decoder
        .read_info()
        .map_err(|source| CharacterError::Decode { name: asset.name, source: Box::new(source) })?;
    let info = reader.info();
    if info.width != POSE_SIZE || info.height != POSE_SIZE {
        return Err(CharacterError::AssetSize {
            name: asset.name,
            width: info.width,
            height: info.height,
        });
    }
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(CharacterError::AssetFormat { name: asset.name });
    }
    let mut image = Pixmap::new(POSE_SIZE, POSE_SIZE).ok_or(CharacterError::RasterAllocation)?;
    if reader.output_buffer_size() != Some(image.data().len()) {
        return Err(CharacterError::AssetFormat { name: asset.name });
    }
    reader
        .next_frame(image.data_mut())
        .map_err(|source| CharacterError::Decode { name: asset.name, source: Box::new(source) })?;
    sampling::premultiply_rgba(image.data_mut());
    Ok(image)
}

/// Match QPainter's integer-position fast path for an unscaled, front-facing pixmap.
///
/// Qt rounds translation-only `drawPixmap` destinations even when smooth transformation
/// is enabled. The original 387-pixel slot places its 386-pixel card at a half pixel; using
/// bilinear filtering there would blur the entire bootstrap character by half a pixel.
fn card_transform(quad: [Point; 4], extent: f64) -> Transform {
    let sx = (quad[1].x - quad[0].x) / extent;
    let ky = (quad[1].y - quad[0].y) / extent;
    let kx = (quad[3].x - quad[0].x) / extent;
    let sy = (quad[3].y - quad[0].y) / extent;
    if (sx - 1.0).abs() < 1e-12 && (sy - 1.0).abs() < 1e-12 && ky.abs() < 1e-12 && kx.abs() < 1e-12
    {
        return Transform::from_translate(quad[0].x.round() as f32, quad[0].y.round() as f32);
    }
    Transform::from_row(
        sx as f32,
        ky as f32,
        kx as f32,
        sy as f32,
        quad[0].x as f32,
        quad[0].y as f32,
    )
}

/// Reproduce the source's short, round-capped paper edge around the flip midpoint.
fn paint_paper_edge(
    target: &mut Pixmap,
    quad: [Point; 4],
    progress: f64,
    scale: f64,
    bounds: Bounds,
) {
    let midpoint_distance = (progress - 0.5).abs();
    if !midpoint_distance.is_finite() || midpoint_distance > 0.035 {
        return;
    }
    let alpha = (255.0 * 0.35 * (1.0 - midpoint_distance / 0.035)) as u8;
    let top = Point { x: (quad[0].x + quad[1].x) / 2.0, y: (quad[0].y + quad[1].y) / 2.0 };
    let bottom = Point { x: (quad[2].x + quad[3].x) / 2.0, y: (quad[2].y + quad[3].y) / 2.0 };
    sampling::paper_edge(target, top, bottom, 1.25 * scale, alpha, bounds);
}

#[cfg(test)]
#[path = "character_tests.rs"]
mod tests;
