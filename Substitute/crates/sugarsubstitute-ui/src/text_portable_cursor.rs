// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Rasterize one uncached portable glyph and budget its authored pixel composition.

use crate::bitmap::Bitmap;
use crate::text::LabelPaintSpec;
use crate::text::memory::{self, Allocation, TEXTURE_BYTES};
use cosmic_text::{Color, FontSystem, PhysicalGlyph, SwashCache, SwashContent, SwashImage};
use std::{
    error::Error,
    fmt, io,
    num::NonZeroUsize,
    sync::{Arc, Mutex, MutexGuard, TryLockError},
};
use sugarsubstitute_contracts::execution::MemoryClaim;
use tiny_skia::{FillRule, Paint, PathBuilder, Rect, Transform};

pub(super) struct Glyph {
    pub(super) physical: PhysicalGlyph,
    pub(super) color: Color,
}

/// Own one portable image at a time; no scale variants accumulate in a persistent cache.
///
/// Font access uses try_lock and never waits. The private owner serializes creation
/// and cursor steps; overlapping use is a typed error rather than an implicit wait.
/// Opaque library rasterization is indivisible and receives one bounded 14*scale
/// glyph. Its returned image must fit the preclaimed four-MiB capacity before use.
pub struct GlyphPaint {
    fonts: Arc<Mutex<FontSystem>>,
    plans: Vec<Glyph>,
    // The library scaler state is opaque like its font allocations; boxing it keeps
    // this cursor's inline size close to the other scene paint variants.
    cache: Box<SwashCache>,
    image: Option<Image>,
    glyph: usize,
    scale: f32,
    origin_y: f32,
    origin_x: f32,
    clip: Option<[i32; 4]>,
    background: Option<crate::text::console::BackgroundPaint>,
    width: u32,
    height: u32,
    allocation: Allocation,
    _metadata: Option<MemoryClaim>,
}

struct Image {
    data: SwashImage,
    position: usize,
    pixels: usize,
    _claim: Option<MemoryClaim>,
}

/// Observe exclusive font access without blocking or recovering from a poisoned owner.
pub(super) fn fonts(owner: &Mutex<FontSystem>) -> io::Result<MutexGuard<'_, FontSystem>> {
    owner.try_lock().map_err(|cause| {
        io::Error::other(match cause {
            TryLockError::WouldBlock => FontAccessError::InUse,
            TryLockError::Poisoned(_) => FontAccessError::Poisoned,
        })
    })
}

/// Retain the failure classification without exporting a borrowed mutex guard.
#[derive(Debug)]
pub(super) enum FontAccessError {
    InUse,
    Poisoned,
}
impl fmt::Display for FontAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InUse => "Status font owner is already in use",
            Self::Poisoned => "Status font owner is poisoned",
        })
    }
}
impl Error for FontAccessError {}

impl GlyphPaint {
    #[cfg(test)]
    pub(super) fn cached_images(&self) -> usize {
        self.cache.image_cache.len()
    }
    pub(super) fn new(
        fonts: Arc<Mutex<FontSystem>>,
        plans: Vec<Glyph>,
        allocation: Allocation,
        metadata: Option<MemoryClaim>,
        spec: LabelPaintSpec,
        origin_y: f32,
    ) -> Self {
        let crate::text::PaintSpec { scale, width, height, .. } = spec.surface;
        Self {
            fonts,
            plans,
            cache: Box::new(SwashCache::new()),
            image: None,
            glyph: 0,
            scale,
            origin_y,
            origin_x: spec.layout.origin[0] * scale,
            clip: spec.layout.clip.map(|_| spec.device_clip()),
            background: None,
            width,
            height,
            allocation,
            _metadata: metadata,
        }
    }

    /// Retain actual cluster geometry while restricting console glyph overhang to its row.
    pub(super) fn bind_console(
        &mut self,
        spec: crate::text::console::ConsolePaintSpec,
        background: crate::text::console::BackgroundPaint,
    ) {
        self.origin_x = spec.origin[0] * self.scale;
        let mut clip = spec.clip();
        clip[1] = clip[1].max((spec.top() * self.scale).round() as i32);
        clip[3] = clip[3].min(((spec.top() + spec.extent[1]) * self.scale).round() as i32);
        self.clip = Some(clip);
        self.background = Some(background);
    }

    /// Visit at most min(max,65536) source pixels or perform one opaque glyph call.
    ///
    /// Returns true once all glyphs are composed. A changed target shape is rejected
    /// before mutation. Every image drops before its claim becomes reusable.
    /// # Errors
    /// Preserves target mismatch, allocation and font raster failures.
    pub fn step(&mut self, target: &mut Bitmap, max: NonZeroUsize) -> io::Result<bool> {
        if target.width() != self.width || target.height() != self.height {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Status target shape changed"));
        }
        if let Some(background) = &mut self.background {
            if background.step(target, max)? {
                self.background = None;
            }
            return Ok(false);
        }
        if self.glyph == self.plans.len() {
            return Ok(true);
        }
        if self.image.is_none() {
            let claim = self.allocation.claim(TEXTURE_BYTES)?;
            let mut fonts = fonts(&self.fonts)?;
            let image = self
                .cache
                .get_image_uncached(&mut fonts, self.plans[self.glyph].physical.cache_key);
            if let Some(data) = image {
                let pixels = (data.placement.width as usize)
                    .checked_mul(data.placement.height as usize)
                    .ok_or_else(|| io::Error::other("Portable glyph shape overflow"))?;
                let stride = match data.content {
                    SwashContent::Mask => 1,
                    SwashContent::Color => 4,
                    SwashContent::SubpixelMask => {
                        return Err(io::Error::other("Unsupported portable subpixel glyph"));
                    }
                };
                let length = pixels
                    .checked_mul(stride)
                    .ok_or_else(|| io::Error::other("Portable glyph length overflow"))?;
                if data.data.capacity() > TEXTURE_BYTES
                    || data.data.len() != length
                    || data.placement.width > 4096
                    || data.placement.height > 4096
                {
                    return Err(io::Error::other("Portable glyph exceeds raster bound"));
                }
                self.image = Some(Image { data, position: 0, pixels, _claim: claim });
            } else {
                self.glyph += 1;
            }
            return Ok(self.glyph == self.plans.len());
        }
        let image =
            self.image.as_mut().ok_or_else(|| io::Error::other("Missing portable glyph"))?;
        let plan = &self.plans[self.glyph];
        let end = image.position.saturating_add(memory::budget(max)).min(image.pixels);
        for index in image.position..end {
            let color = match image.data.content {
                SwashContent::Mask => Color::rgba(
                    plan.color.r(),
                    plan.color.g(),
                    plan.color.b(),
                    ((u16::from(image.data.data[index]) * u16::from(plan.color.a()) + 127) / 255)
                        as u8,
                ),
                SwashContent::Color => {
                    let p = &image.data.data[index * 4..index * 4 + 4];
                    Color::rgba(p[0], p[1], p[2], p[3])
                }
                SwashContent::SubpixelMask => {
                    return Err(io::Error::other("Unsupported portable subpixel glyph"));
                }
            };
            let x = i64::from(plan.physical.x)
                + i64::from(image.data.placement.left)
                + (index % image.data.placement.width as usize) as i64;
            let y = i64::from(plan.physical.y) - i64::from(image.data.placement.top)
                + (index / image.data.placement.width as usize) as i64;
            let device_x = self.origin_x + x as f32;
            let device_y = self.origin_y + y as f32;
            if self.clip.is_some_and(|clip| {
                device_x < clip[0] as f32
                    || device_y < clip[1] as f32
                    || device_x >= clip[2] as f32
                    || device_y >= clip[3] as f32
            }) {
                continue;
            }
            if let Some(rect) = Rect::from_xywh(device_x, device_y, 1.0, 1.0) {
                // Preserve portable pixel placement and tiny-skia blending while
                // keeping the authored outer loop bounded by the caller's budget.
                let mut paint = Paint { anti_alias: false, ..Paint::default() };
                paint.set_color_rgba8(color.r(), color.g(), color.b(), color.a());
                target.fill_path(
                    &PathBuilder::from_rect(rect),
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
        }
        image.position = end;
        if end == image.pixels {
            self.image = None;
            self.glyph += 1;
        }
        Ok(self.glyph == self.plans.len())
    }
}
