// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Draw presentation overlays, the selection's marching ants, a drawn shape's outline, transform
//! and shared-edge guides and the smart guides of held snaps, over display pixels.
//!
//! Overlay geometry is projected through the presented camera and rasterized into opaque
//! one-pixel strokes, as CuteCanvas's cosmetic pens draw them. Dash positions follow device-pixel
//! distance along each stroke; each selection boundary run starts its own dash, and the light
//! dashes of every run are drawn before the dark ones, as CuteCanvas's two pen passes are. Smart
//! guides are drawn last, over everything else. Pixel storage follows the strokes actually drawn
//! and is admitted as it grows. The same pixels are painted into CPU display buffers and drawn by
//! the GPU display pass, so both presentations show identical marks, and animating the ants only
//! rasterizes the overlay again, never document pixels.
use crate::{CpuProjection, RenderError, cpu::check};
use sugarsubstitute_canvas::{
    ANT_DARK, ANT_LIGHT, GUIDE, GUIDE_DISABLED, GUIDE_HANDLE, GUIDE_HANDLE_RADIUS, OverlayGuides,
    OverlayOutline, PresentationOverlay, SNAP_GUIDE, SeamHandle, Viewport, outline_light,
};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
pub use sugarsubstitute_image_gpu::OverlayPixel;
use sugarsubstitute_image_ops::{Extent, ImageError};

mod geometry;
use geometry::trace;

const WHITE: [u8; 3] = [255, 255, 255];
const BLACK: [u8; 3] = [0, 0, 0];
const PIXEL_CHUNK: usize = 4096;

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
    context: Option<&'a JobContext>,
    maximum: Option<usize>,
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
        let mut raster = Raster {
            extent,
            pixels: Vec::new(),
            memory: Vec::new(),
            context: Some(context),
            maximum: None,
        };
        trace(overlay, view, &mut raster)?;
        check(context)?;
        Ok(Self { extent, pixels: raster.pixels, _memory: raster.memory })
    }
    /// Conservative pixel-capacity admission for this exact overlay and camera, including the
    /// raster owner's allocation chunks. Reuses the same geometry traversal and clipped segment
    /// support, while skipping pixel sampling, dash filtering and duplicate suppression.
    /// No raster pixel storage or execution reservation is allocated by this query.
    /// # Errors
    /// Preserves camera failures and rejects a count exceeding the addressable pixel inventory.
    pub fn maximum_pixels(
        overlay: &PresentationOverlay,
        view: Viewport,
    ) -> Result<usize, RenderError> {
        let mut raster = Raster {
            extent: view.surface(),
            pixels: Vec::new(),
            memory: Vec::new(),
            context: None,
            maximum: Some(0),
        };
        trace(overlay, view, &mut raster)?;
        let count =
            raster.maximum.ok_or(RenderError::Invalid("Overlay estimate lost its count"))?;
        count
            .checked_add(PIXEL_CHUNK - 1)
            .map(|count| count / PIXEL_CHUNK * PIXEL_CHUNK)
            .ok_or(RenderError::Invalid("Overlay estimate exceeds addressable pixels"))
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

impl Raster<'_> {
    /// Draw a transform box with its eight handles, or shared edges with their three handles,
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
                    let emphasized = |handle| seam.focused && seam.handle == Some(handle);
                    let handles = [
                        (start, emphasized(SeamHandle::Start)),
                        (middle, seam.focused),
                        (end, emphasized(SeamHandle::End)),
                    ];
                    for ((point, emphasis), enabled) in handles.into_iter().zip(seam.enabled) {
                        let radius = GUIDE_HANDLE_RADIUS + if emphasis { 1.0 } else { 0.0 };
                        if enabled {
                            self.handle(point, radius)?;
                        } else {
                            self.disabled_handle(point, radius)?;
                        }
                    }
                }
            }
        }
        self.control()
    }
    /// A grey handle crossed out, for a handle that cannot be dragged.
    fn disabled_handle(&mut self, center: [f64; 2], radius: f64) -> Result<(), RenderError> {
        self.disc(center, radius, GUIDE_DISABLED)?;
        let inset = radius * 0.55;
        let cross = Stroke::Solid(GUIDE_HANDLE);
        let [x, y] = center;
        self.segment([x - inset, y - inset], [x + inset, y + inset], 0.0, cross, None, None)?;
        self.segment([x - inset, y + inset], [x + inset, y - inset], 0.0, cross, None, None)?;
        Ok(())
    }
    /// A filled circular handle with a one-pixel guide-coloured rim.
    fn handle(&mut self, center: [f64; 2], radius: f64) -> Result<(), RenderError> {
        self.disc(center, radius, GUIDE_HANDLE)
    }
    /// A disc filled with `fill` inside a one-pixel guide-coloured rim.
    fn disc(&mut self, [x, y]: [f64; 2], radius: f64, fill: [u8; 3]) -> Result<(), RenderError> {
        let [width, height] = [f64::from(self.extent.width()), f64::from(self.extent.height())];
        let reach = radius + 1.0;
        let (left, top) = ((x - reach).floor().max(0.0), (y - reach).floor().max(0.0));
        let (right, bottom) = ((x + reach).ceil().min(width), (y + reach).ceil().min(height));
        if self.maximum.is_some() {
            let columns = (right - left).max(0.0) as usize;
            let rows = (bottom - top).max(0.0) as usize;
            self.add_maximum(columns.checked_mul(rows).ok_or(RenderError::Invalid(
                "Overlay handle estimate exceeds addressable pixels",
            ))?)?;
            return Ok(());
        }
        let mut row = top;
        while row < bottom {
            let mut column = left;
            while column < right {
                let distance = (column + 0.5 - x).hypot(row + 0.5 - y);
                let color = if distance <= radius - 0.5 {
                    Some(fill)
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
        if self.maximum.is_some() {
            self.add_maximum(usize::try_from(last.saturating_sub(first)).map_err(|_| {
                RenderError::Invalid("Overlay segment estimate exceeds addressable pixels")
            })?)?;
            return Ok(start + length);
        }
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
    fn control(&self) -> Result<(), RenderError> {
        self.context.map_or(Ok(()), check)
    }
    fn add_maximum(&mut self, count: usize) -> Result<(), RenderError> {
        let maximum =
            self.maximum.as_mut().ok_or(RenderError::Invalid("Overlay count mode is absent"))?;
        *maximum = maximum
            .checked_add(count)
            .ok_or(RenderError::Invalid("Overlay estimate exceeds addressable pixels"))?;
        Ok(())
    }
    fn push(&mut self, pixel: OverlayPixel) -> Result<(), RenderError> {
        if self.maximum.is_some() {
            return self.add_maximum(1);
        }
        if self.pixels.len() == self.pixels.capacity() {
            self.control()?;
            let claim = self
                .context
                .ok_or(RenderError::Invalid("Overlay raster worker is absent"))?
                .memory()
                .claim((PIXEL_CHUNK * size_of::<OverlayPixel>()) as u64)
                .map_err(ImageError::from)?;
            self.pixels.try_reserve_exact(PIXEL_CHUNK).map_err(ImageError::from)?;
            self.memory.push(claim);
        }
        self.pixels.push(pixel);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
