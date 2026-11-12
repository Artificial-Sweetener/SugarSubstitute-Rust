// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Keep pose decoding, coverage and card sampling resumable without changing arithmetic.

use super::{CharacterError, CharacterLibrary, POSE_SIZE, decode::PoseDecoder, sampling};
use crate::animation::AnimationFrame;
use crate::projection::{Bounds, Point, projected_quad};
use sugarsubstitute_ui::bitmap::{Bitmap, BitmapView};
use tiny_skia::{FilterQuality, PixmapPaint, Transform};

/// Own cursor state while transiently borrowing the renderer cache for each step.
pub(crate) struct CharacterRender {
    frame: AnimationFrame,
    bounds: Bounds,
    quad: [Point; 4],
    scale: f64,
    transform: Transform,
    sampling: Option<sampling::SamplingCursor>,
    decoder: Option<PoseDecoder>,
    initialized: bool,
    image_done: bool,
    image_row: u32,
    edge_row: u32,
    height: u32,
}
impl CharacterRender {
    /// Compute the authoritative logical projection before composing device coordinates.
    pub(crate) fn new(
        width: u32,
        height: u32,
        bounds: Bounds,
        frame: AnimationFrame,
    ) -> Result<Self, CharacterError> {
        if !bounds.is_valid()
            || [bounds.x, bounds.y, bounds.width, bounds.height]
                .iter()
                .any(|value| value.abs() > f64::from(f32::MAX))
        {
            return Err(CharacterError::InvalidBounds);
        }
        let card_size = bounds.width.min(bounds.height);
        let extent = f64::from(POSE_SIZE);
        let scale = card_size / extent;
        let logical_quad = projected_quad(
            Bounds {
                x: (bounds.width - card_size) / (2.0 * scale),
                y: (bounds.height - card_size) / (2.0 * scale),
                width: extent,
                height: extent,
            },
            frame.progress,
            frame.direction,
        );
        let quad = logical_quad
            .map(|point| Point { x: bounds.x + point.x * scale, y: bounds.y + point.y * scale });
        let transform = card_transform(quad, extent);
        let fast = transform.sx == 1.0
            && transform.sy == 1.0
            && transform.kx == 0.0
            && transform.ky == 0.0;
        let sampling = if fast {
            None
        } else {
            let mapping = sampling::DeviceTransform::from_quad(
                logical_quad,
                extent,
                Point { x: bounds.x / scale, y: bounds.y / scale },
                scale,
            );
            Some(sampling::SamplingCursor::new(
                width as usize,
                height as usize,
                quad,
                bounds,
                mapping,
            )?)
        };
        Ok(Self {
            frame,
            bounds,
            quad,
            scale,
            transform,
            sampling,
            decoder: None,
            initialized: false,
            image_done: false,
            image_row: 0,
            edge_row: 0,
            height,
        })
    }
    /// Read one PNG row, advance one coverage slice, or paint at most eight physical rows.
    pub(crate) fn step(
        &mut self,
        library: &mut CharacterLibrary,
        target: &mut Bitmap,
    ) -> Result<bool, CharacterError> {
        let index = self.frame.pose;
        let asset = *library.assets.get(index).ok_or(CharacterError::PoseIndex { index })?;
        let slot = library.decoded.get_mut(index).ok_or(CharacterError::PoseIndex { index })?;
        if !self.initialized {
            self.initialized = true;
            if slot.is_none() {
                self.decoder = Some(PoseDecoder::new(asset)?);
            }
            return Ok(false);
        }
        if let Some(decoder) = &mut self.decoder {
            if decoder.step()? {
                let decoder = self.decoder.take().ok_or(CharacterError::PoseIndex { index })?;
                *slot = Some(decoder.finish()?);
            }
            return Ok(false);
        }
        let source = slot.as_ref().ok_or(CharacterError::PoseIndex { index })?;
        if !self.image_done {
            if let Some(cursor) = &mut self.sampling {
                self.image_done = cursor.step(target, source)?;
            } else {
                let end = (self.image_row + 8).min(source.height());
                let stride = source.width() as usize * 4;
                let strip = BitmapView::from_bytes(
                    &source.data()[self.image_row as usize * stride..end as usize * stride],
                    source.width(),
                    end - self.image_row,
                )
                .ok_or(CharacterError::InvalidBounds)?;
                target.draw_bitmap(
                    0,
                    self.image_row as i32,
                    strip,
                    &PixmapPaint { quality: FilterQuality::Bilinear, ..PixmapPaint::default() },
                    self.transform,
                    None,
                );
                self.image_row = end;
                self.image_done = end == source.height();
            }
            return Ok(false);
        }
        let distance = (self.frame.progress - 0.5).abs();
        if !distance.is_finite() || distance > 0.035 {
            return Ok(true);
        }
        let alpha = (255.0 * 0.35 * (1.0 - distance / 0.035)) as u8;
        let top = Point {
            x: (self.quad[0].x + self.quad[1].x) / 2.0,
            y: (self.quad[0].y + self.quad[1].y) / 2.0,
        };
        let bottom = Point {
            x: (self.quad[2].x + self.quad[3].x) / 2.0,
            y: (self.quad[2].y + self.quad[3].y) / 2.0,
        };
        let end = (self.edge_row + 8).min(self.height);
        let y = f64::from(self.edge_row).max(self.bounds.y.round());
        let y1 = f64::from(end).min((self.bounds.y + self.bounds.height).round());
        if y < y1 {
            sampling::paper_edge(
                target,
                top,
                bottom,
                1.25 * self.scale,
                alpha,
                Bounds { x: self.bounds.x, y, width: self.bounds.width, height: y1 - y },
            );
        }
        self.edge_row = end;
        Ok(end == self.height)
    }
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
