// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Decode one reviewed glyph row per step and retain the selected caption cache pair.

use super::{ASSETS, CaptionIcon, CaptionIcons, IconError};
use crate::packaged_png::{PngError, PngRows};
use tiny_skia::{FilterQuality, IntSize, Pixmap, PixmapPaint, Transform};

/// Capture icon selection without decoding, cloning or tinting on the submitting thread.
pub(crate) struct CaptionCursor {
    index: usize,
    kind: CaptionIcon,
    tint: [u8; 3],
    scale: f64,
    decoder: Option<PngRows>,
    pair: Vec<Pixmap>,
    initialized: bool,
    glyph: Option<Pixmap>,
    painted: bool,
}
impl CaptionCursor {
    /// Select the nearest reviewed resource scale with the original tie order.
    pub(crate) fn new(scale: f64, kind: CaptionIcon, tint: [u8; 3]) -> Result<Self, IconError> {
        if !scale.is_finite() || !(0.5..=4.0).contains(&scale) {
            return Err(IconError::InvalidScale);
        }
        let index = ASSETS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (a.0 - scale).abs().total_cmp(&(b.0 - scale).abs()))
            .map_or(0, |(index, _)| index);
        Ok(Self {
            index,
            kind,
            tint,
            scale,
            decoder: None,
            pair: Vec::new(),
            initialized: false,
            glyph: None,
            painted: false,
        })
    }
    /// Decode one row or tint/paint one fixed glyph of at most 23552 physical pixels.
    pub(crate) fn step(
        &mut self,
        cache: &mut CaptionIcons,
        target: &mut Pixmap,
    ) -> Result<bool, IconError> {
        if self.painted {
            return Ok(true);
        }
        if !self.initialized {
            self.initialized = true;
            if cache.decoded.as_ref().is_none_or(|(index, _)| *index != self.index) {
                self.pair.try_reserve_exact(2).map_err(error)?;
                self.decoder = Some(self.decoder(0)?);
            }
            return Ok(false);
        }
        if let Some(decoder) = &mut self.decoder {
            if decoder.step().map_err(png_error)? {
                let decoder = self.decoder.take().ok_or(IconError::InvalidScale)?;
                self.pair.push(decoder.finish().map_err(png_error)?);
            }
            return Ok(false);
        }
        if self.pair.len() == 1 {
            self.decoder = Some(self.decoder(1)?);
            return Ok(false);
        }
        if self.pair.len() == 2 {
            let close = self.pair.pop().ok_or(IconError::InvalidScale)?;
            let console = self.pair.pop().ok_or(IconError::InvalidScale)?;
            cache.decoded = Some((self.index, [console, close]));
            return Ok(false);
        }
        let (position, offset) = match self.kind {
            CaptionIcon::Console => (0, 466.0),
            CaptionIcon::Close => (1, 512.0),
        };
        if self.glyph.is_none() {
            let (_, icons) = cache.decoded.as_ref().ok_or(IconError::InvalidScale)?;
            let source = &icons[position];
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(source.data().len()).map_err(error)?;
            if bytes.capacity() != source.data().len() {
                return Err(IconError::InvalidScale);
            }
            bytes.extend_from_slice(source.data());
            for pixel in bytes.chunks_exact_mut(4) {
                let alpha = pixel[3];
                for (channel, tint) in pixel[..3].iter_mut().zip(self.tint) {
                    *channel = crate::shapes::byte_product(tint, alpha);
                }
            }
            let size =
                IntSize::from_wh(source.width(), source.height()).ok_or(IconError::InvalidScale)?;
            self.glyph = Some(Pixmap::from_vec(bytes, size).ok_or(IconError::InvalidScale)?);
            return Ok(false);
        }
        let glyph = self.glyph.as_ref().ok_or(IconError::InvalidScale)?;
        let ratio = (self.scale / ASSETS[self.index].0) as f32;
        target.draw_pixmap(
            0,
            0,
            glyph.as_ref(),
            &PixmapPaint { quality: FilterQuality::Bilinear, ..PixmapPaint::default() },
            Transform::from_row(ratio, 0.0, 0.0, ratio, (offset * self.scale).floor() as f32, 0.0),
            None,
        );
        self.painted = true;
        Ok(true)
    }
    /// Validate reviewed dimensions before decoding each member of the replacement pair.
    fn decoder(&self, position: usize) -> Result<PngRows, IconError> {
        let (scale, console, close) = ASSETS[self.index];
        PngRows::new(
            if position == 0 { console } else { close },
            (46.0 * scale).round() as u32,
            (32.0 * scale).round() as u32,
        )
        .map_err(png_error)
    }
}
/// Retain exact allocator/decoder causes without exposing the private PNG owner publicly.
fn error(source: impl std::error::Error + Send + Sync + 'static) -> IconError {
    IconError::Preparation(Box::new(source))
}

/// Preserve the public decoder variant while retaining new format/capacity failures.
fn png_error(source: PngError) -> IconError {
    match source {
        PngError::Decode(source) => IconError::Decode(source),
        source => error(source),
    }
}
