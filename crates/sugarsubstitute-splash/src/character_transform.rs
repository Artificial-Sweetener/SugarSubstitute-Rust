// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain logical projection and device composition before fixed-point texture sampling.
//!
//! Numerical evaluation follows Qt 6.11's native QTransform and raster transform contracts.

use crate::projection::Point;

/// Own the six affine texture coefficients after native transform composition.
#[derive(Clone, Copy)]
pub(crate) struct DeviceTransform {
    pub(crate) x_axis: Point,
    pub(crate) y_axis: Point,
    pub(crate) origin: Point,
}

impl DeviceTransform {
    /// Map a logical source square into a widget before adding its device scale and origin.
    pub(crate) fn from_quad(quad: [Point; 4], extent: f64, origin: Point, scale: f64) -> Self {
        let [a, b, c, d] = quad;
        let residual = Point { x: a.x - b.x + c.x - d.x, y: a.y - b.y + c.y - d.y };
        let (x_axis, y_axis) = if residual.x == 0.0 && residual.y == 0.0 {
            (Point { x: b.x - a.x, y: b.y - a.y }, Point { x: c.x - b.x, y: c.y - b.y })
        } else {
            // Small parallelogram residuals affect control coefficients even when the
            // native transform classifies the resulting projective terms as zero.
            let one = Point { x: b.x - c.x, y: b.y - c.y };
            let two = Point { x: d.x - c.x, y: d.y - c.y };
            let determinant = one.x * two.y - two.x * one.y;
            let g = (residual.x * two.y - two.x * residual.y) / determinant;
            let h = (one.x * residual.y - residual.x * one.y) / determinant;
            (
                Point { x: b.x - a.x + g * b.x, y: b.y - a.y + g * b.y },
                Point { x: d.x - a.x + h * d.x, y: d.y - a.y + h * d.y },
            )
        };
        let source_scale = 1.0 / extent;
        Self {
            x_axis: Point {
                x: (source_scale * x_axis.x) * scale,
                y: (source_scale * x_axis.y) * scale,
            },
            y_axis: Point {
                x: (source_scale * y_axis.x) * scale,
                y: (source_scale * y_axis.y) * scale,
            },
            origin: Point { x: a.x * scale + origin.x * scale, y: a.y * scale + origin.y * scale },
        }
    }

    /// Bias logical source coordinates before inversion, preserving native cancellation order.
    pub(crate) fn inverse(self) -> Self {
        let Self { x_axis: a, y_axis: b, origin } = self;
        let epsilon = 1.0 / 65_536.0;
        let origin = Point {
            x: (epsilon * a.x + epsilon * b.x) + origin.x,
            y: (epsilon * a.y + epsilon * b.y) + origin.y,
        };
        if a.y.abs() <= 1e-12 && b.x.abs() <= 1e-12 {
            let (x, y) = (1.0 / a.x, 1.0 / b.y);
            return Self {
                x_axis: Point { x, y: 0.0 },
                y_axis: Point { x: 0.0, y },
                origin: Point { x: -origin.x * x, y: -origin.y * y },
            };
        }
        let reciprocal = 1.0 / (a.x * b.y - b.x * a.y);
        Self {
            x_axis: Point { x: b.y * reciprocal, y: -a.y * reciprocal },
            y_axis: Point { x: -b.x * reciprocal, y: a.x * reciprocal },
            origin: Point {
                x: (b.x * origin.y - b.y * origin.x) * reciprocal,
                y: (a.y * origin.x - a.x * origin.y) * reciprocal,
            },
        }
    }

    /// Inverse-map a physical pixel center in the native multiply-and-add order.
    pub(crate) fn map(self, x: f64, y: f64) -> Point {
        Point {
            x: self.y_axis.x * y + self.x_axis.x * x + self.origin.x,
            y: self.y_axis.y * y + self.x_axis.y * x + self.origin.y,
        }
    }
}
