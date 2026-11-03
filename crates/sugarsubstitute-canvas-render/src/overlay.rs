// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Draw presentation overlays, the selection's marching ants and a drawn shape's outline, over
//! display pixels.
//!
//! Overlay geometry is projected through the presented camera and rasterized into opaque
//! one-pixel strokes, as CuteCanvas's cosmetic pens draw them. Dash positions follow device-pixel
//! distance along each stroke; each selection boundary run starts its own dash, and the light
//! dashes of every run are drawn before the dark ones, as CuteCanvas's two pen passes are. Pixel
//! storage follows the strokes actually drawn and is admitted as it grows. The same pixels are painted into CPU display buffers and drawn by
//! the GPU display pass, so both presentations show identical marks, and animating the ants only
//! rasterizes the overlay again, never document pixels.
use crate::{CpuProjection, RenderError, cpu::check};
use sugarsubstitute_canvas::{
    ANT_DARK, ANT_LIGHT, GUIDE, GUIDE_HANDLE, GUIDE_HANDLE_RADIUS, OverlayGuides, OverlayOutline,
    PresentationOverlay, Viewport, outline_light,
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
pub use sugarsubstitute_image_gpu::OverlayPixel;
use sugarsubstitute_image_ops::{Extent, ImageError};

const WHITE: [u8; 3] = [255, 255, 255];
const BLACK: [u8; 3] = [0, 0, 0];

#[derive(Clone, Copy)]
enum Stroke {
    Ants(sugarsubstitute_canvas::MarchingAnts),
    Outline,
    Solid([u8; 3]),
}
impl Stroke {
    fn color(self, distance: f64) -> [u8; 3] {
        match self {
            Self::Ants(ants) if ants.light(distance) => ANT_LIGHT,
            Self::Ants(_) => ANT_DARK,
            Self::Outline if outline_light(distance) => WHITE,
            Self::Outline => BLACK,
            Self::Solid(color) => color,
        }
    }
}

/// Overlay pixels rasterized for one physical viewport, drawn in order.
pub struct OverlayPixels {
    extent: Extent,
    pixels: Vec<OverlayPixel>,
    _memory: Vec<MemoryClaim>,
}

/// Admitted pixel accumulation for one viewport.
struct Raster<'a> {
    extent: Extent,
    pixels: Vec<OverlayPixel>,
    memory: Vec<MemoryClaim>,
    context: &'a JobContext,
}

impl OverlayPixels {
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
        if let Some(boundary) = &overlay.selection {
            let [dx, dy] = overlay.selection_offset.map(|value| value as f64);
            let stroke = Stroke::Ants(overlay.ants);
            for only in [ANT_LIGHT, ANT_DARK] {
                for run in boundary.runs() {
                    let point =
                        |[x, y]: [u32; 2]| view.device_at([f64::from(x) + dx, f64::from(y) + dy]);
                    let (from, to) = (point(run.start)?, point(run.end())?);
                    raster.segment(from, to, 0.0, stroke, None, Some(only))?;
                }
                check(context)?;
            }
        }
        if let Some(outline) = &overlay.outline {
            let document = overlay.document;
            let [left, top] = view.device_at([0.0, 0.0])?;
            let [right, bottom] =
                view.device_at([f64::from(document.width()), f64::from(document.height())])?;
            let clip = Some([left.floor(), top.floor(), right.ceil(), bottom.ceil()]);
            let points = outline_points(outline, view)?;
            match outline {
                OverlayOutline::Line(_) => {
                    raster.segment(points[0], points[1], 0.0, Stroke::Outline, clip, None)?;
                }
                _ => raster.closed(&points, Stroke::Outline, clip)?,
            }
        }
        if let Some(guides) = &overlay.guides {
            raster.guides(guides, view)?;
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

/// A shape's outline in device coordinates: a closed polyline, or a line's two end points.
/// Ellipses are flattened in device space so they stay smooth at every zoom.
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
        OverlayOutline::Line([from, to]) => vec![view.device_at(*from)?, view.device_at(*to)?],
    })
}

impl Raster<'_> {
    /// Draw a transform box with its eight handles, or shared edges with their middle handles,
    /// as CuteCanvas's cosmetic guide pens and circular affine handles.
    fn guides(&mut self, guides: &OverlayGuides, view: Viewport) -> Result<(), RenderError> {
        let solid = Stroke::Solid(GUIDE);
        match guides {
            OverlayGuides::Frame(corners) => {
                let mut points = [[0.0; 2]; 4];
                for (point, corner) in points.iter_mut().zip(corners) {
                    *point = view.device_at(*corner)?;
                }
                self.closed(&points, solid, None)?;
                for (index, corner) in points.iter().enumerate() {
                    let next = points[(index + 1) % points.len()];
                    let middle = [(corner[0] + next[0]) / 2.0, (corner[1] + next[1]) / 2.0];
                    self.handle(*corner, GUIDE_HANDLE_RADIUS)?;
                    self.handle(middle, GUIDE_HANDLE_RADIUS)?;
                }
            }
            OverlayGuides::Seams(seams) => {
                for seam in seams.iter() {
                    let (start, end) = (view.device_at(seam.start)?, view.device_at(seam.end)?);
                    self.segment(start, end, 0.0, solid, None, None)?;
                    if seam.focused {
                        // A focused seam is two device pixels wide.
                        let along = [end[0] - start[0], end[1] - start[1]];
                        let length = along[0].hypot(along[1]).max(f64::EPSILON);
                        let shift = [-along[1] / length, along[0] / length];
                        let offset = |[x, y]: [f64; 2]| [x + shift[0], y + shift[1]];
                        self.segment(offset(start), offset(end), 0.0, solid, None, None)?;
                    }
                    let middle = [(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0];
                    let radius = GUIDE_HANDLE_RADIUS + if seam.focused { 1.0 } else { 0.0 };
                    self.handle(middle, radius)?;
                }
            }
        }
        check(self.context)
    }
    /// A filled circular handle with a one-pixel guide-coloured rim.
    fn handle(&mut self, [x, y]: [f64; 2], radius: f64) -> Result<(), RenderError> {
        let [width, height] = [f64::from(self.extent.width()), f64::from(self.extent.height())];
        let reach = radius + 1.0;
        let (left, top) = ((x - reach).floor().max(0.0), (y - reach).floor().max(0.0));
        let (right, bottom) = ((x + reach).ceil().min(width), (y + reach).ceil().min(height));
        let mut row = top;
        while row < bottom {
            let mut column = left;
            while column < right {
                let distance = (column + 0.5 - x).hypot(row + 0.5 - y);
                let color = if distance <= radius - 0.5 {
                    Some(GUIDE_HANDLE)
                } else if distance <= radius + 0.5 {
                    Some(GUIDE)
                } else {
                    None
                };
                if let Some(color) = color {
                    self.push(OverlayPixel { x: column as u32, y: row as u32, color })?;
                }
                column += 1.0;
            }
            row += 1.0;
        }
        Ok(())
    }
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
            distance = self.segment(*from, to, distance, stroke, clip, None)?;
        }
        Ok(())
    }
    /// Stroke one segment by sampling it once per device pixel along its major axis, keeping
    /// only samples of colour `only` when given, and return the dash distance at its end.
    fn segment(
        &mut self,
        from: [f64; 2],
        to: [f64; 2],
        start: f64,
        stroke: Stroke,
        clip: Option<[f64; 4]>,
        only: Option<[u8; 3]>,
    ) -> Result<f64, RenderError> {
        let delta = [to[0] - from[0], to[1] - from[1]];
        let length = delta[0].hypot(delta[1]);
        let [width, height] = [f64::from(self.extent.width()), f64::from(self.extent.height())];
        let outside = from[0].max(to[0]) < 0.0
            || from[1].max(to[1]) < 0.0
            || from[0].min(to[0]) >= width
            || from[1].min(to[1]) >= height;
        if length == 0.0 || outside {
            return Ok(start + length);
        }
        let steps = delta[0].abs().max(delta[1].abs()).ceil().max(1.0);
        // Visit only the samples near the surface, however far a magnified stroke extends.
        let (mut low, mut high) = (0.0_f64, 1.0_f64);
        for (axis, limit) in [width, height].into_iter().enumerate() {
            if delta[axis] != 0.0 {
                let enter = (-1.0 - from[axis]) / delta[axis];
                let leave = (limit + 1.0 - from[axis]) / delta[axis];
                low = low.max(enter.min(leave));
                high = high.min(enter.max(leave));
            }
        }
        if low >= high {
            return Ok(start + length);
        }
        let first = (low * steps - 0.5).floor().max(0.0) as u64;
        let last = ((high * steps - 0.5).ceil() + 1.0).min(steps) as u64;
        for step in first..last {
            let t = (step as f64 + 0.5) / steps;
            let [x, y] = [(from[0] + delta[0] * t).floor(), (from[1] + delta[1] * t).floor()];
            let clipped = clip.is_some_and(|[left, top, right, bottom]| {
                x < left || y < top || x >= right || y >= bottom
            });
            if clipped || x < 0.0 || y < 0.0 || x >= width || y >= height {
                continue;
            }
            let (x, y) = (x as u32, y as u32);
            // Consecutive polyline segments share their joint pixel; draw it once.
            if only.is_none() && self.pixels.last().is_some_and(|last| (last.x, last.y) == (x, y)) {
                continue;
            }
            let color = stroke.color(start + length * t);
            if only.is_none_or(|only| only == color) {
                self.push(OverlayPixel { x, y, color })?;
            }
        }
        Ok(start + length)
    }
    fn push(&mut self, pixel: OverlayPixel) -> Result<(), RenderError> {
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
