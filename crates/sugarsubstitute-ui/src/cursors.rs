// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Render canvas editing cursors as device pixels for a native host to wrap in cursor handles.
//!
//! The brush ring, crosshairs and their add/erase marks reproduce CuteCanvas's cursor builder:
//! a black, white and colour ring 5, 3 and 1 logical pixels wide, a 32 pixel crosshair with a
//! 5 pixel gap and a 4 pixel padding, and an underscore erase mark. Selection-translate and
//! move-cut use SugarSubstitute's Fluent-derived artwork at a 40 pixel base size with its
//! hotspot at (6, 3). Every size here is logical and scales by the display scale; outputs are
//! straight-alpha RGBA, the format native cursor resources take.

use crate::vector::{VectorAsset, VectorViewport};
use std::io;
use tiny_skia::{Color, LineCap, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

const PADDING: f32 = 4.0;
const CROSSHAIR: f32 = 32.0;
const CROSSHAIR_GAP: f32 = 5.0;
const ARTWORK_SIZE: f32 = 40.0;
const ARTWORK_HOTSPOT: [f32; 2] = [6.0, 3.0];
const SELECTION_TRANSLATE: &str = include_str!("../assets/cursors/SelectionTranslateCursor.svg");
const MOVE_CUT: &str = include_str!("../assets/cursors/MoveCutCursor.svg");

/// Which canvas cursor to draw.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CursorArt {
    /// A brush ring `diameter` physical pixels across in an sRGB `color`, optionally with the
    /// erase mark. The diameter is already in device pixels, so it does not scale again.
    BrushOutline {
        /// Ring diameter in physical pixels, at least 2.
        diameter: u32,
        /// Ring colour as sRGB bytes.
        color: [u8; 3],
        /// Whether to draw the erase mark.
        erase: bool,
    },
    /// The precise crosshair with an optional mode mark.
    Precise(PreciseMark),
    /// Selection outline translation and layer move artwork.
    SelectionTranslate,
    /// Move-cut artwork.
    MoveCut,
}

/// Mark drawn inside the precise crosshair.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PreciseMark {
    /// A plain crosshair.
    None,
    /// The addition badge at the lower right.
    Add,
    /// The erase underscore at the lower right.
    Subtract,
}

/// One rendered cursor: straight-alpha RGBA rows and a hotspot inside the image.
#[derive(Clone, Debug, PartialEq)]
pub struct CursorImage {
    width: u32,
    height: u32,
    hotspot: [u32; 2],
    rgba: Vec<u8>,
}

impl CursorImage {
    /// Width in physical pixels.
    pub const fn width(&self) -> u32 {
        self.width
    }
    /// Height in physical pixels.
    pub const fn height(&self) -> u32 {
        self.height
    }
    /// Hotspot in physical pixels from the top-left corner.
    pub const fn hotspot(&self) -> [u32; 2] {
        self.hotspot
    }
    /// Straight-alpha RGBA bytes, top row first.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

impl CursorArt {
    /// Largest ring a host may request, in physical pixels.
    pub const MAX_RING_DIAMETER: u32 = 2048;

    /// Render at `scale` physical pixels per logical pixel (the display DPI over 96).
    /// # Errors
    /// Rejects a nonfinite or nonpositive scale, a ring outside 2 to
    /// [`Self::MAX_RING_DIAMETER`] pixels, and allocation or artwork failures.
    pub fn render(self, scale: f32) -> io::Result<CursorImage> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err(invalid("Cursor scale must be finite and positive"));
        }
        match self {
            Self::BrushOutline { diameter, color, erase } => ring(diameter, color, erase, scale),
            Self::Precise(mark) => crosshair(mark, scale),
            Self::SelectionTranslate => artwork(SELECTION_TRANSLATE, scale),
            Self::MoveCut => artwork(MOVE_CUT, scale),
        }
    }
}

fn ring(diameter: u32, color: [u8; 3], erase: bool, scale: f32) -> io::Result<CursorImage> {
    if !(2..=CursorArt::MAX_RING_DIAMETER).contains(&diameter) {
        return Err(invalid("Brush outline diameter is outside the cursor range"));
    }
    let border = (PADDING * scale).round().max(1.0);
    let edge = diameter + 2 * border as u32;
    let mut pixmap = pixmap(edge, edge)?;
    let size = diameter as f32;
    // CuteCanvas insets the ellipse by half a logical pixel and shrinks it by one so the
    // widest ring stays inside the padded image.
    let center = border + size / 2.0;
    let radius = ((size - scale) / 2.0).max(0.5);
    let circle = PathBuilder::from_circle(center, center, radius)
        .ok_or_else(|| invalid("Brush outline geometry is empty"))?;
    for (ring_color, width) in [([0, 0, 0], 5.0), ([255, 255, 255], 3.0), (color, 1.0)] {
        let stroke = Stroke { width: width * scale, ..Stroke::default() };
        pixmap.stroke_path(&circle, &solid(ring_color), &stroke, Transform::identity(), None);
    }
    if erase {
        erase_mark(&mut pixmap, [border, border], size / scale, scale);
    }
    let hotspot = border as u32 + diameter / 2;
    Ok(image(pixmap, [hotspot, hotspot]))
}

fn crosshair(mark: PreciseMark, scale: f32) -> io::Result<CursorImage> {
    let logical = CROSSHAIR + 2.0 * PADDING;
    let edge = (logical * scale).round() as u32;
    let mut pixmap = pixmap(edge, edge)?;
    let transform = Transform::from_scale(scale, scale).pre_translate(PADDING, PADDING);
    let center = CROSSHAIR / 2.0;
    let inset = 2.0;
    let mut lines = PathBuilder::new();
    for [start, end] in [
        [[center, inset], [center, center - CROSSHAIR_GAP]],
        [[center, center + CROSSHAIR_GAP], [center, CROSSHAIR - inset]],
        [[inset, center], [center - CROSSHAIR_GAP, center]],
        [[center + CROSSHAIR_GAP, center], [CROSSHAIR - inset, center]],
    ] {
        lines.move_to(start[0], start[1]);
        lines.line_to(end[0], end[1]);
    }
    let lines = lines.finish().ok_or_else(|| invalid("Crosshair geometry is empty"))?;
    // Qt pens default to square caps; the white pen outlines the black one.
    for (color, width) in [([255, 255, 255], 4.0), ([0, 0, 0], 2.0)] {
        let stroke = Stroke { width, line_cap: LineCap::Square, ..Stroke::default() };
        pixmap.stroke_path(&lines, &solid(color), &stroke, transform, None);
    }
    match mark {
        PreciseMark::None => {}
        PreciseMark::Add => add_mark(&mut pixmap, transform)?,
        PreciseMark::Subtract => {
            let origin = PADDING * scale;
            erase_mark(&mut pixmap, [origin, origin], CROSSHAIR, scale);
        }
    }
    let hotspot = ((PADDING + center) * scale).round() as u32;
    Ok(image(pixmap, [hotspot, hotspot]))
}

/// The outlined plus badge CuteCanvas draws at the crosshair's lower right.
fn add_mark(pixmap: &mut Pixmap, transform: Transform) -> io::Result<()> {
    let center = CROSSHAIR - 5.0;
    let mut plus = PathBuilder::new();
    plus.move_to(center - 3.0, center);
    plus.line_to(center + 3.0, center);
    plus.move_to(center, center - 3.0);
    plus.line_to(center, center + 3.0);
    let plus = plus.finish().ok_or_else(|| invalid("Add mark geometry is empty"))?;
    for (color, width) in [([255, 255, 255], 4.0), ([0, 0, 0], 2.0)] {
        let stroke = Stroke { width, line_cap: LineCap::Round, ..Stroke::default() };
        pixmap.stroke_path(&plus, &solid(color), &stroke, transform, None);
    }
    Ok(())
}

/// CuteCanvas's erase mark: an underscore at 30% of the feedback extent in points, aligned to
/// the lower right of a rectangle inset by 10% and shifted by (15%, 10%) of the extent, drawn
/// black and then white one pixel up and left. The underscore is drawn as its glyph's bar:
/// half an em wide and a fifteenth of an em thick, resting just above the line's bottom.
/// `origin` is in physical pixels; `extent` is logical.
fn erase_mark(pixmap: &mut Pixmap, origin: [f32; 2], extent: f32, scale: f32) {
    let extent = extent.max(1.0);
    let points = (extent * 0.3).floor().max(4.0);
    let em = points * 96.0 / 72.0;
    let padding = (extent * 0.1).floor();
    let right = extent * 0.15 + extent - padding;
    let bottom = extent * 0.1 + extent - padding - em * 0.07;
    let width = em * 0.5;
    let thickness = (em / 15.0).max(1.0);
    for (offset, color) in [(0.0, [0, 0, 0]), (-1.0, [255, 255, 255])] {
        let left = origin[0] + (right - width + offset) * scale;
        let top = origin[1] + (bottom - thickness + offset) * scale;
        if let Some(bar) = Rect::from_xywh(left, top, width * scale, thickness * scale) {
            pixmap.fill_rect(bar, &solid(color), Transform::identity(), None);
        }
    }
}

fn artwork(source: &'static str, scale: f32) -> io::Result<CursorImage> {
    let scale = scale.clamp(0.5, 4.0);
    let asset = VectorAsset::parse(source)?;
    let rendered = asset.rasterize(
        VectorViewport { x: 0.0, y: 0.0, width: ARTWORK_SIZE, height: ARTWORK_SIZE, scale },
        None,
    )?;
    let view = rendered.pixels();
    let pixmap = Pixmap::from_vec(
        view.data().to_vec(),
        tiny_skia::IntSize::from_wh(view.width(), view.height())
            .ok_or_else(|| invalid("Cursor artwork is empty"))?,
    )
    .ok_or_else(|| invalid("Cursor artwork buffer changed shape"))?;
    let hotspot = ARTWORK_HOTSPOT.map(|value| (value * scale).round() as u32);
    Ok(image(pixmap, hotspot))
}

/// A transparent image whose allocation failure is reported rather than aborting.
fn pixmap(width: u32, height: u32) -> io::Result<Pixmap> {
    let size = tiny_skia::IntSize::from_wh(width, height)
        .ok_or_else(|| invalid("Cursor image size is unsupported"))?;
    let bytes = usize::try_from(u64::from(width) * u64::from(height) * 4)
        .map_err(|_| invalid("Cursor image size is unsupported"))?;
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(bytes).map_err(io::Error::other)?;
    pixels.resize(bytes, 0);
    Pixmap::from_vec(pixels, size).ok_or_else(|| invalid("Cursor image size is unsupported"))
}

fn solid([red, green, blue]: [u8; 3]) -> Paint<'static> {
    let mut paint = Paint { anti_alias: true, ..Paint::default() };
    paint.set_color(Color::from_rgba8(red, green, blue, 255));
    paint
}

fn image(pixmap: Pixmap, hotspot: [u32; 2]) -> CursorImage {
    let (width, height) = (pixmap.width(), pixmap.height());
    let hotspot = [hotspot[0].min(width - 1), hotspot[1].min(height - 1)];
    CursorImage { width, height, hotspot, rgba: pixmap.take_demultiplied() }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
#[path = "cursors_tests.rs"]
mod tests;
