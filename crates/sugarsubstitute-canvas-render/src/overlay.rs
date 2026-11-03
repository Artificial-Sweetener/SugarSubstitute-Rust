// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Draw presentation overlays, the selection's marching ants and a drawn shape's outline, over
//! display pixels.
//!
//! Overlay geometry is projected through the presented camera and rasterized into opaque
//! one-pixel strokes, as CuteCanvas's cosmetic pens draw them. Dash positions follow device-pixel
//! distance along each outline. The same pixels are painted into CPU display buffers and drawn by
//! the GPU display pass, so both presentations show identical marks, and animating the ants only
//! rasterizes the overlay again, never document pixels.
use crate::{CpuProjection, RenderError, cpu::check};
use sugarsubstitute_canvas::{
    ANT_DARK, ANT_LIGHT, OverlayOutline, PresentationOverlay, Viewport, outline_light,
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_gpu::MAX_OVERLAY_PIXELS;
pub use sugarsubstitute_image_gpu::OverlayPixel;
use sugarsubstitute_image_ops::{Extent, ImageError};

const WHITE: [u8; 3] = [255, 255, 255];
const BLACK: [u8; 3] = [0, 0, 0];

#[derive(Clone, Copy)]
enum Stroke {
    Ants(sugarsubstitute_canvas::MarchingAnts),
    Outline,
}
impl Stroke {
    fn color(self, distance: f64) -> [u8; 3] {
        match self {
            Self::Ants(ants) if ants.light(distance) => ANT_LIGHT,
            Self::Ants(_) => ANT_DARK,
            Self::Outline if outline_light(distance) => WHITE,
            Self::Outline => BLACK,
        }
    }
}

/// Overlay pixels rasterized for one physical viewport, drawn in order.
pub struct OverlayPixels {
    extent: Extent,
    pixels: Vec<OverlayPixel>,
    _memory: Vec<MemoryClaim>,
}

/// Bounded pixel accumulation for one viewport, clipped to an optional device rectangle.
struct Raster<'a> {
    extent: Extent,
    pixels: Vec<OverlayPixel>,
    memory: Vec<MemoryClaim>,
    context: &'a JobContext,
}

impl OverlayPixels {
    /// Most pixels one overlay draws; strokes beyond it are left out.
    pub const MAX: usize = MAX_OVERLAY_PIXELS;
    /// Project and rasterize an overlay for `view`'s physical surface on an admitted worker.
    /// Outlines being drawn are clipped to the document; selection ants are not, so an outline
    /// along the document edge stays visible.
    /// # Errors
    /// Preserves cancellation, deadline, memory and camera failures.
    pub fn rasterize(
        overlay: &PresentationOverlay,
        view: Viewport,
        context: &JobContext,
    ) -> Result<Self, RenderError> {
        check(context)?;
        let extent = view.surface();
        let mut raster = Raster { extent, pixels: Vec::new(), memory: Vec::new(), context };
        if let Some(contours) = &overlay.selection {
            let [dx, dy] = overlay.selection_offset.map(|value| value as f64);
            for contour in contours.contours() {
                check(context)?;
                let bytes = (contour.points().len() * size_of::<[f64; 2]>()) as u64;
                let _points = context.memory().claim(bytes + 64).map_err(ImageError::from)?;
                let mut points = Vec::new();
                points.try_reserve_exact(contour.points().len()).map_err(ImageError::from)?;
                for [x, y] in contour.points() {
                    points.push(view.device_at([f64::from(*x) + dx, f64::from(*y) + dy])?);
                }
                raster.closed(&points, Stroke::Ants(overlay.ants), None)?;
            }
        }
        if let Some(outline) = &overlay.outline {
            let document = overlay.document;
            let [left, top] = view.device_at([0.0, 0.0])?;
            let [right, bottom] =
                view.device_at([f64::from(document.width()), f64::from(document.height())])?;
            let clip = Some([left.floor(), top.floor(), right.ceil(), bottom.ceil()]);
            let points = outline_points(outline, view)?;
            raster.closed(&points, Stroke::Outline, clip)?;
        }
        Ok(Self { extent, pixels: raster.pixels, _memory: raster.memory })
    }
    /// Physical surface the pixels were rasterized for.
    pub const fn extent(&self) -> Extent {
        self.extent
    }
    /// Pixels in drawing order; later pixels cover earlier ones.
    pub fn pixels(&self) -> &[OverlayPixel] {
        &self.pixels
    }
    /// Paint the pixels into row-major `0xffRRGGBB` words of the same surface.
    /// # Errors
    /// Rejects a buffer of another extent.
    pub fn paint(&self, extent: Extent, target: &mut [u32]) -> Result<(), RenderError> {
        if extent != self.extent || target.len() as u64 != extent.pixels() {
            return Err(RenderError::Invalid("Overlay differs from its display surface"));
        }
        let stride = extent.width() as usize;
        for pixel in &self.pixels {
            let [red, green, blue] = pixel.color;
            target[pixel.y as usize * stride + pixel.x as usize] =
                u32::from_be_bytes([255, red, green, blue]);
        }
        Ok(())
    }
}

impl CpuProjection {
    /// Paint overlay pixels over these disposable display pixels.
    /// # Errors
    /// Rejects an overlay rasterized for another surface.
    pub fn paint_overlay(&mut self, overlay: &OverlayPixels) -> Result<(), RenderError> {
        let extent = self.view().target().surface();
        overlay.paint(extent, self.pixels_mut())
    }
}

/// A shape's closed outline in device coordinates; ellipses are flattened in device space so
/// they stay smooth at every zoom.
fn outline_points(outline: &OverlayOutline, view: Viewport) -> Result<Vec<[f64; 2]>, RenderError> {
    Ok(match outline {
        OverlayOutline::Rectangle([left, top, right, bottom]) => vec![
            view.device_at([*left, *top])?,
            view.device_at([*right, *top])?,
            view.device_at([*right, *bottom])?,
            view.device_at([*left, *bottom])?,
        ],
        OverlayOutline::Ellipse([left, top, right, bottom]) => {
            let [x0, y0] = view.device_at([*left, *top])?;
            let [x1, y1] = view.device_at([*right, *bottom])?;
            let centre = [(x0 + x1) / 2.0, (y0 + y1) / 2.0];
            let radii = [(x1 - x0) / 2.0, (y1 - y0) / 2.0];
            let perimeter = std::f64::consts::TAU * radii[0].max(radii[1]);
            let segments = ((perimeter / 2.0).ceil() as usize).clamp(16, 8192);
            (0..segments)
                .map(|index| {
                    let angle = std::f64::consts::TAU * index as f64 / segments as f64;
                    [centre[0] + radii[0] * angle.cos(), centre[1] + radii[1] * angle.sin()]
                })
                .collect()
        }
        OverlayOutline::Polygon(points) => {
            points.iter().map(|point| view.device_at(*point)).collect::<Result<_, _>>()?
        }
    })
}

impl Raster<'_> {
    /// Stroke a closed device-space polyline, measuring dash distance from its first point.
    fn closed(
        &mut self,
        points: &[[f64; 2]],
        stroke: Stroke,
        clip: Option<[f64; 4]>,
    ) -> Result<(), RenderError> {
        let mut distance = 0.0;
        for (index, from) in points.iter().enumerate() {
            let to = points[(index + 1) % points.len()];
            distance = self.segment(*from, to, distance, stroke, clip)?;
        }
        Ok(())
    }
    /// Stroke one segment by sampling it once per device pixel along its major axis, returning
    /// the dash distance at its end.
    fn segment(
        &mut self,
        from: [f64; 2],
        to: [f64; 2],
        start: f64,
        stroke: Stroke,
        clip: Option<[f64; 4]>,
    ) -> Result<f64, RenderError> {
        let delta = [to[0] - from[0], to[1] - from[1]];
        let length = delta[0].hypot(delta[1]);
        let [width, height] = [f64::from(self.extent.width()), f64::from(self.extent.height())];
        let outside = from[0].max(to[0]) < 0.0
            || from[1].max(to[1]) < 0.0
            || from[0].min(to[0]) >= width
            || from[1].min(to[1]) >= height;
        if length == 0.0 || outside || self.pixels.len() >= OverlayPixels::MAX {
            return Ok(start + length);
        }
        let steps = delta[0].abs().max(delta[1].abs()).ceil().max(1.0);
        for step in 0..steps as u64 {
            let t = (step as f64 + 0.5) / steps;
            let [x, y] = [(from[0] + delta[0] * t).floor(), (from[1] + delta[1] * t).floor()];
            let clipped = clip.is_some_and(|[left, top, right, bottom]| {
                x < left || y < top || x >= right || y >= bottom
            });
            if clipped || x < 0.0 || y < 0.0 || x >= width || y >= height {
                continue;
            }
            let (x, y) = (x as u32, y as u32);
            if self.pixels.last().is_some_and(|last| (last.x, last.y) == (x, y)) {
                continue;
            }
            self.push(OverlayPixel { x, y, color: stroke.color(start + length * t) })?;
        }
        Ok(start + length)
    }
    fn push(&mut self, pixel: OverlayPixel) -> Result<(), RenderError> {
        if self.pixels.len() >= OverlayPixels::MAX {
            return Ok(());
        }
        if self.pixels.len() == self.pixels.capacity() {
            const CHUNK: usize = 4096;
            check(self.context)?;
            let claim = self
                .context
                .memory()
                .claim((CHUNK * size_of::<OverlayPixel>()) as u64)
                .map_err(ImageError::from)?;
            self.pixels.try_reserve_exact(CHUNK).map_err(ImageError::from)?;
            self.memory.push(claim);
        }
        self.pixels.push(pixel);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
