// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A document-space camera independent of native windows, input devices and image storage.
use crate::{CanvasError, DamagePlan};
use sugarsubstitute_image_ops::Extent;

const MIN_ZOOM: f64 = 1e-9;
const MAX_ZOOM: f64 = 16384.0;

/// Immutable camera state. Origin is the document coordinate at the device viewport's top-left.
/// Zoom is physical device pixels per document pixel; device scale is handled at the input boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    surface: Extent,
    origin: [f64; 2],
    zoom: f64,
    translation: [i32; 2],
}
impl Viewport {
    /// Map source device coordinates directly without cancelling the retained integer phase.
    pub(crate) fn projection_to(self, target: Self) -> Result<(f64, [f64; 2]), CanvasError> {
        let scale = target.zoom / self.zoom;
        let offset = std::array::from_fn(|axis| {
            (self.origin[axis] - target.origin[axis]) * target.zoom
                + f64::from(target.translation[axis])
                - f64::from(self.translation[axis]) * scale
        });
        if !scale.is_finite() || scale <= 0.0 {
            return Err(CanvasError::Invalid("Unrepresentable viewport projection scale"));
        }
        finite(offset)?;
        Ok((scale, offset))
    }
    /// Create an actual-size view with its document origin at the surface origin.
    /// # Errors
    /// Rejects surfaces beyond the 16384-pixel edge bound; image/document extents are independent.
    pub fn new(surface: Extent) -> Result<Self, CanvasError> {
        validate_surface(surface)?;
        Ok(Self { surface, origin: [0.0; 2], zoom: 1.0, translation: [0; 2] })
    }
    /// Current drawable dimensions in physical device pixels.
    pub const fn surface(self) -> Extent {
        self.surface
    }
    /// Document coordinates at the physical viewport's top-left pixel corner.
    pub const fn origin(self) -> [f64; 2] {
        [
            self.origin[0] - self.translation[0] as f64 / self.zoom,
            self.origin[1] - self.translation[1] as f64 / self.zoom,
        ]
    }
    /// Physical device pixels per document pixel.
    pub const fn zoom(self) -> f64 {
        self.zoom
    }
    /// Map a finite device coordinate to document space without rounding.
    /// # Errors
    /// Rejects nonfinite input or an unrepresentable result.
    pub fn document_at(self, device: [f64; 2]) -> Result<[f64; 2], CanvasError> {
        finite(device)?;
        let result = [
            self.origin[0] + (device[0] - f64::from(self.translation[0])) / self.zoom,
            self.origin[1] + (device[1] - f64::from(self.translation[1])) / self.zoom,
        ];
        finite(result)?;
        Ok(result)
    }
    /// Map a finite document coordinate to physical device space.
    /// # Errors
    /// Rejects nonfinite input or an unrepresentable result.
    pub fn device_at(self, document: [f64; 2]) -> Result<[f64; 2], CanvasError> {
        finite(document)?;
        let result = self.project(document);
        finite(result)?;
        Ok(result)
    }
    /// Pan content by device-pixel deltas, preserving document state and zoom.
    /// # Errors
    /// Rejects nonfinite deltas or an unrepresentable resulting origin, without mutation.
    pub fn pan_by(&mut self, device_delta: [f64; 2]) -> Result<(), CanvasError> {
        finite(device_delta)?;
        if device_delta
            .iter()
            .all(|delta| delta.fract() == 0.0 && delta.abs() <= f64::from(i32::MAX))
        {
            let next = [
                i64::from(self.translation[0]) + device_delta[0] as i64,
                i64::from(self.translation[1]) + device_delta[1] as i64,
            ];
            if let (Ok(x), Ok(y)) = (i32::try_from(next[0]), i32::try_from(next[1])) {
                self.translation = [x, y];
                return Ok(());
            }
        }
        let previous = self.origin();
        let origin =
            [previous[0] - device_delta[0] / self.zoom, previous[1] - device_delta[1] / self.zoom];
        finite(origin)?;
        self.origin = origin;
        self.translation = [0; 2];
        Ok(())
    }
    /// Change scale while preserving the document point beneath a device-space anchor.
    /// # Errors
    /// Rejects nonpositive/nonfinite factors or unrepresentable coordinates without mutation.
    pub fn zoom_at(&mut self, factor: f64, anchor: [f64; 2]) -> Result<(), CanvasError> {
        if !factor.is_finite() || factor <= 0.0 {
            return Err(CanvasError::Invalid("Invalid zoom factor"));
        }
        self.set_zoom((self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM), anchor)
    }
    /// Set an exact document-space camera without changing authored content.
    /// `origin` is the top-left document coordinate; `zoom` is physical pixels per document pixel.
    /// An unchanged camera preserves its integer sampling phase.
    /// # Errors
    /// Rejects nonfinite coordinates or unsupported scale without mutation.
    pub fn reframe(&mut self, origin: [f64; 2], zoom: f64) -> Result<(), CanvasError> {
        finite(origin)?;
        if !zoom.is_finite() || !(MIN_ZOOM..=MAX_ZOOM).contains(&zoom) {
            return Err(CanvasError::Invalid("Invalid camera scale"));
        }
        if self.zoom != zoom || self.origin() != origin {
            self.origin = origin;
            self.zoom = zoom;
            self.translation = [0; 2];
        }
        Ok(())
    }
    /// Display one document pixel per physical device pixel, preserving the anchor point.
    /// # Errors
    /// Rejects invalid anchor coordinates without changing the view.
    pub fn actual_size(&mut self, anchor: [f64; 2]) -> Result<(), CanvasError> {
        self.set_zoom(1.0, anchor)
    }
    /// Center the complete image in the current viewport with no crop.
    /// # Errors
    /// Reports a scale outside supported camera precision without changing the view.
    pub fn fit(&mut self, image: Extent) -> Result<(), CanvasError> {
        let zoom = (f64::from(self.surface.width()) / f64::from(image.width()))
            .min(f64::from(self.surface.height()) / f64::from(image.height()));
        if !(MIN_ZOOM..=MAX_ZOOM).contains(&zoom) {
            return Err(CanvasError::Invalid("Image fit scale exceeds camera bounds"));
        }
        let origin = [
            (f64::from(image.width()) - f64::from(self.surface.width()) / zoom) * 0.5,
            (f64::from(image.height()) - f64::from(self.surface.height()) / zoom) * 0.5,
        ];
        self.origin = origin;
        self.zoom = zoom;
        self.translation = [0; 2];
        Ok(())
    }
    /// Resize the drawable viewport while retaining the document point at its center.
    /// # Errors
    /// Rejects unsupported dimensions without changing the view.
    pub fn resize(&mut self, surface: Extent) -> Result<(), CanvasError> {
        validate_surface(surface)?;
        if self.surface == surface {
            return Ok(());
        }
        let center = self.document_at([
            f64::from(self.surface.width()) * 0.5,
            f64::from(self.surface.height()) * 0.5,
        ])?;
        let origin = [
            center[0] - f64::from(surface.width()) * 0.5 / self.zoom,
            center[1] - f64::from(surface.height()) * 0.5 / self.zoom,
        ];
        finite(origin)?;
        self.surface = surface;
        self.origin = origin;
        self.translation = [0; 2];
        Ok(())
    }
    /// Plan reuse against the actually presented camera when document content is unchanged.
    /// Exact integer translation is required; fractional translation never blits stale sample phases.
    /// # Errors
    /// Preserves checked region-construction failures before presenting a repair plan.
    pub fn repair_from(self, previous: Self) -> Result<DamagePlan, CanvasError> {
        if self.surface != previous.surface
            || self.zoom != previous.zoom
            || self.origin != previous.origin
        {
            return Ok(DamagePlan::full(self.surface));
        }
        let x = i64::from(self.translation[0]) - i64::from(previous.translation[0]);
        let y = i64::from(self.translation[1]) - i64::from(previous.translation[1]);
        let (Ok(x), Ok(y)) = (i32::try_from(x), i32::try_from(y)) else {
            return Ok(DamagePlan::full(self.surface));
        };
        DamagePlan::pan(self.surface, x, y)
    }
    fn set_zoom(&mut self, zoom: f64, anchor: [f64; 2]) -> Result<(), CanvasError> {
        finite(anchor)?;
        if zoom == self.zoom {
            return Ok(());
        }
        let point = self.document_at(anchor)?;
        let origin = [point[0] - anchor[0] / zoom, point[1] - anchor[1] / zoom];
        finite(origin)?;
        self.origin = origin;
        self.zoom = zoom;
        self.translation = [0; 2];
        Ok(())
    }
    /// Camera phase before its exact physical-pixel displacement.
    pub(crate) const fn untranslated(self) -> Self {
        Self { translation: [0; 2], ..self }
    }
    /// Physical-pixel displacement subtracted before raster and vector sample conversion.
    pub(crate) const fn pixel_translation(self) -> [i32; 2] {
        self.translation
    }
    /// Test the document footprint in the same untranslated device grid as analytic coverage.
    /// Infinite distant projections indicate an offscreen footprint; no visible pixels are omitted.
    pub(crate) fn intersects_document(self, document: Extent) -> bool {
        let base = self.untranslated();
        let start = base.project([0.0; 2]);
        let end = base.project([f64::from(document.width()), f64::from(document.height())]);
        let left = -i64::from(self.translation[0]);
        let top = -i64::from(self.translation[1]);
        let right = i64::from(self.surface.width()) - i64::from(self.translation[0]);
        let bottom = i64::from(self.surface.height()) - i64::from(self.translation[1]);
        !(start[0] >= right as f64
            || start[1] >= bottom as f64
            || end[0] <= left as f64
            || end[1] <= top as f64)
    }
    fn project(self, document: [f64; 2]) -> [f64; 2] {
        [
            (document[0] - self.origin[0]) * self.zoom + f64::from(self.translation[0]),
            (document[1] - self.origin[1]) * self.zoom + f64::from(self.translation[1]),
        ]
    }
}

fn finite(point: [f64; 2]) -> Result<(), CanvasError> {
    if point.iter().any(|v| !v.is_finite()) {
        return Err(CanvasError::Invalid("Nonfinite canvas coordinate"));
    }
    Ok(())
}
fn validate_surface(surface: Extent) -> Result<(), CanvasError> {
    if surface.width() > 16384 || surface.height() > 16384 {
        return Err(CanvasError::Invalid("Canvas surface exceeds device-pixel bounds"));
    }
    Ok(())
}
